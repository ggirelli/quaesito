<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo-dark.png">
    <img src="assets/brand/logo-light.png" alt="quaesito" width="480">
  </picture>
</h1>

Quizzes among friends: a self-hosted, live quiz web app written in Rust with [Dioxus](https://dioxuslabs.com).

- **Admin** builds quizzes in the browser: questions with optional pictures and soundtracks, time limits, and answers as
  multiple choice (2–6 answers, one or several correct), a typed answer, a number (closest guess wins) or items to put
  in order.
- **Big screen** shows the quiz as a slideshow on a TV or projector, with a QR code to join. Slides with a title, text
  or a picture can sit between questions, e.g. to announce rounds.
- **Team leaders** join from their phones by scanning the QR code or typing the game code, pick a team name and a picture, and answer against the clock. Faster correct answers earn more points.
- **Podium** at the end reveals 3rd, 2nd and finally 1st place.

It runs on your local network and needs no internet connection.

## Playing

```sh
QUAESITO_ADMIN_PASSWORD=pick-something just play
```

Then:

1. Open `http://localhost:8080` on the laptop driving the TV, click the ⚙ icon in the top right corner and log in.
2. Create a quiz, or import one (JSON). The editor saves as you type and tells you when the quiz is ready to play.
3. Press **▶ Start**. The big screen shows a QR code and a 6-digit code.
4. Team leaders scan the code (phones must be on the same Wi-Fi), choose a name and optionally a picture.
5. Press **Start the quiz**, then use the button, `→`, `PageDown` or `N` (presentation clickers work) to move on.
   `←`, `PageUp` or `B` go back: to the previous slide, results or podium step, or out of a question opened by mistake
   (its answers are discarded and it is asked again later). **⏸ Pause** (`P`) freezes the clock, the music and the
   automatic advance; **+10 s** gives everyone more time.

The big screen plays sound effects (a ticking last five seconds, reveal chimes, a drumroll and fanfare on the podium);
🔊 in its top bar mutes them. Browsers only allow sound after a first click or key press on the page.

**Teams** in the top bar of the big screen lets you rename a team or remove it (it loses its points and its phone goes
back to the join page).

A question's picture can be **revealed gradually**: it starts heavily blurred and sharpens as the clock runs out, so
early guesses are harder but, with speed points, worth more.

A question can have a soundtrack (MP3, M4A/AAC, Ogg/Opus, WebM, FLAC or WAV, up to 40 MB). It then
waits on the big screen with a full clock until the host presses **▶ Start the music**: the music
plays from the big screen's speakers and the countdown starts with it. This also holds with
automatic advance. Phones show "Listen up!" until then.

If phones cannot connect, allow incoming connections on the port in the laptop's firewall. If the detected address is
wrong (VPNs, several network cards), set `QUAESITO_PUBLIC_URL`, e.g. `http://192.168.1.20:8080`.

### Question types

- **Choices**: coloured buttons; one or several answers can be correct. Each answer can have a picture (flags, logos,
  baby photos…), with or without text.
- **Typed answer**: teams type a short answer. It is right when it matches an accepted answer, ignoring case, accents,
  punctuation and spacing (but not typos). On the reveal, identical answers are grouped and the host can judge each
  group: ✓ full, ½ half or ✗ no points, plus a bonus or malus in steps of 100. A malus can take a team below zero.
- **Number**: teams enter a number; the closest guesses (ties included) earn 1000 points.
- **Order**: teams put 2–6 items in order (oldest first, biggest first…) by tapping them on their phones. Each item in
  its right place earns its share of the points.

Between questions you can add **slides** (title, text and/or picture) to introduce rounds or explain rules. Phones show
"Eyes on the big screen" meanwhile, and with automatic advance a slide stays up for 10 seconds.

**🔀 Shuffle questions** in the editor reorders the questions at random, round by round: slides stay in place and
questions never move past a slide. The last shuffle can be undone until you make another change.

### Quiz settings

Each quiz chooses:

| Setting | Options |
|---|---|
| Reveal answers | after each question (with standings: points gained, answer times, places moved) · at the end (walk through the answers before the podium) |
| Several correct answers | all or nothing · partial credit (correct picks minus wrong picks) |
| Points | speed (faster correct answers earn more) · correct answers only (1000 each) |
| Jokers | off · on: each team may play a joker once, while answering, to double that question's points |
| Moving on | host clicks · automatic after a short pause |
| On team phones | coloured buttons only (read on the big screen) · full question and answer text |

With speed points, a correct answer earns 1000 points when instant, down to 500 at the buzzer. With correct answers
only, it always earns 1000. Either way, ties are broken by the time spent on correct answers.

### Configuration

| Variable | Default | Meaning |
|---|---|---|
| `QUAESITO_ADMIN_PASSWORD` | generated and printed at startup | Admin password |
| `QUAESITO_DATA_DIR` | `./data` | Where quizzes (JSON) and pictures are stored |
| `QUAESITO_PUBLIC_URL` | LAN address + port | Base URL used in join links and QR codes |
| `IP` / `PORT` | `127.0.0.1` / `8080` | Listening address of the server binary |

Live games are kept in memory, so restarting the server ends them. Quizzes are plain files you can back up.

## Development

Requires Rust (stable), the `wasm32-unknown-unknown` target, [`dx`](https://dioxuslabs.com/learn/0.7/getting_started) (Dioxus CLI 0.7), [`just`](https://github.com/casey/just) and [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny).

```sh
just serve   # hot-reloading dev server on the LAN (admin password defaults to "quaesito")
just check   # fmt + clippy (server and wasm) + tests + cargo-deny, same as CI
just play    # release build served on the LAN
```

See [DESIGN.md](DESIGN.md) for how it works and [STATUS.md](STATUS.md) for progress.
