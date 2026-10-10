# Design

## Overview

Quaesito is a single Rust binary (Dioxus 0.7 fullstack, axum underneath) meant to run on a LAN.
It serves three views:

| View | Route | Device | Purpose |
|---|---|---|---|
| Admin | `/admin` | laptop | Password-protected quiz editor; start sessions |
| Host | `/host/<session>` | projector/TV | Slideshow: join QR code, lobby, questions, reveal, podium |
| Player | `/join/<code>` | phone | Team leader picks name + avatar, answers questions |

The admin password guards `/admin` and `/host/<code>`; the host screen is opened from the admin
dashboard when a quiz is started. Phones join at `/join/<code>`.

The server is authoritative for game state and timing. Each running session is an in-memory state
machine (`src/server/engine.rs`) that takes the current time as an input, so it is fully unit tested:

```
Lobby → Question(0) → Closed(0) → … → Closed(n-1) → [Review(0) → … → Review(n-1)] → Podium(0..=3)
```

A question with a soundtrack is preceded by `Ready(i)`: the big screen shows it with a full clock
and the host's next step plays the music and starts the countdown together (never automatic).
A slide in the list shows as `Slide(i)` in place of a question: there is nothing to answer, phones
say to watch the big screen, and the review skips it. With automatic advance it stays up for 10
seconds. `Review` only exists when answers are revealed at the end. A question closes when its time runs
out, when every connected team has answered, or when the host stops the clock. With automatic
advance, a per-session timer task moves on after a short pause.

The host can pause (`HostCommand::TogglePause`): the game keeps a `paused_at` instant, its clock
reads that instant until resumed, and resuming shifts every phase timestamp by the pause length, so
deadlines, auto-advance and answer times all exclude it. Answers sent while paused still count, timed
at the pause. `AddTime` moves an open question's deadline. `Back` shows the previous item again
(a slide, or a played question's results via `Closed`), steps the review and podium backwards, and
withdraws a question that is still open (its answers are dropped). Each question remembers whether
it has been closed, so going forward again shows its results instead of asking it twice.

The host can rename or remove a team at any time before the podium (`HostCommand::RenameTeam`,
`RemoveTeam`). A removed team's answers go with it; its phone's socket closes, the stored ticket no
longer checks out, and the phone returns to the join form with a notice.

Screens hold a WebSocket each (`/api/ws/host`, `/api/ws/player`). On every change the server
pushes a complete, role-specific view (`src/protocol.rs`), so clients never reconstruct state and
a reconnecting phone simply receives the current view. Countdowns are sent as "milliseconds left"
and run locally from the moment the view arrived.

## Quiz model

A quiz has an ordered list of questions. Each question has text, an optional picture (optionally
revealed gradually: the big screen blurs it in proportion to the clock left), an optional soundtrack, a time limit
(defaulting to the quiz-wide one) and a kind. The list also holds slides (`Slide { title }`, with
the question text as body and the picture), which separate rounds or explain rules; they are
numbered apart from questions and score nothing. A quiz needs at least one real question.

| Kind | Teams answer with | Right answer |
|---|---|---|
| `Choice` (default) | coloured buttons | two or more answers (text and/or picture), any of which may be marked correct |
| `Text { accepted }` | a short typed text (≤ 60 characters) | equal to an accepted answer once normalised |
| `Number { answer }` | a number | the closest guess (ties included) |
| `Order { items }` | the items, shown shuffled, tapped from first to last | `items` as listed |
| `Slide { title }` | nothing | none: a title, text and/or picture on the big screen |

Typed answers are normalised before matching: Unicode NFKD, accents dropped, lowercase, and every
run of non-alphanumeric characters turned into a single space. There is no typo tolerance; the
judge amends instead. Answers travel as a `Response` (`Choice(indexes)`, `Text`, `Number`), and the
reveal is per kind: pick counts, typed answers grouped by normalised text, or guesses sorted by
distance.

Per-quiz settings chosen by the admin:

| Setting | Options |
|---|---|
| Reveal | `AfterEachQuestion` / `AtEnd` |
| Multi-correct scoring | `AllOrNothing` / `Partial` |
| Points | `Speed` (faster is worth more) / `Flat` (every correct answer is worth the same) |
| Jokers | off / on (`joker: bool`) |
| Advance | `Manual` (host clicks) / `Auto` (when time runs out or all teams answered) |
| Phone display | `ButtonsOnly` (read on the projector) / `FullText` |

## Scoring

With `Speed` points, a correct answer submitted after `t` seconds on a question with limit `T`
earns `round(1000 * (1 - 0.5 * t / T))` points: 1000 when instant, 500 at the buzzer. With `Flat`
points it always earns 1000. Partial credit and the judge's grades scale this amount.

With `Partial` scoring on multi-correct questions, the fraction
`max(0, (correct picked - wrong picked) / total correct)` multiplies the speed score.

Typed answers: each group of identical (normalised) answers gets a `Judgement`, a grade
(`Full` ×1, `Half` ×0.5, `Wrong` ×0) applied to the speed score, plus a bonus or malus of at most
±1000 points. It starts as `Full` or `Wrong` from the accepted answers; the host can amend it
(`HostCommand::Judge`) whenever that question's answers are shown, after the question or in the
end-of-quiz review. Scores are therefore signed.

Closest number: the closest guesses earn 1000 points each, regardless of speed.

Order: each game shuffles an order question's items once (never leaving them as listed). Phones
answer with indexes into the shuffled list; the speed score is multiplied by the share of items
put in their right place.

Joker: with jokers on, a team may send `joker: true` with one answer in the whole game; that
question's points (bonus or malus included) are doubled. Withdrawing the question by going back
returns the joker. Phones get the joker's state in `PlayerView.joker`.

Points are computed from the stored responses and judgements when needed, so amendments update the
standings immediately. Only closed questions count, so standings never give away a result early.

Ties in the final ranking are broken by lower total response time on correct answers.

Reveals name the fastest fully correct team (not for number questions, where speed does not
count). The standings shown after a question carry each team's `RoundResult`: points gained, answer
time and its rank before that question, computed from the standings one item earlier.

## Storage

Everything lives under a data directory (`QUAESITO_DATA_DIR`, default `./data`):

- `quizzes/<id>.json`: one JSON document per quiz. A quiz is always edited and saved as a whole,
  so plain files written atomically (temp file + rename) are simpler than a database.
- `media/<random>.{jpg,png,mp3,…}`: uploaded pictures and soundtracks. Question pictures are downscaled to fit
  1600×1200 and re-encoded as JPEG; avatars are cropped square, resized to 256×256 and stored as
  PNG. Re-encoding strips metadata and anything that is not plain pixels. Soundtracks are stored
  as uploaded once their format is recognised from their first bytes; `/media` answers byte-range
  requests so the big screen can seek (a reloaded host page resumes the music where the clock is).
- Running sessions take a snapshot of the quiz at start, so editing never affects a live game.
  Sessions are kept in memory only.
- JSON export/import of quizzes for backup and sharing (pictures are not included).

## Networking

Plain HTTP on the LAN, no internet required (no CDNs or web fonts). The join link uses
`QUAESITO_PUBLIC_URL` if set, otherwise the machine's outbound LAN address plus the port the host
page was opened on; the QR code is drawn in the browser. Team leaders get a reconnect token stored
in `localStorage`, so a sleeping phone can rejoin without losing its score.
