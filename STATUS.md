# Status

Living tracker of the implementation plan. See [DESIGN.md](DESIGN.md) for the design itself.

## Plan

| # | Step | Status |
|---|---|---|
| 0 | Repo, CI, `just` recipes, cargo-deny, logo | ✅ done |
| 1 | Quiz model, per-quiz settings, validation | ✅ done |
| 2 | Scoring and ranking | ✅ done |
| 3 | Storage (quizzes on disk) and image handling | ✅ done |
| 4 | Admin auth and quiz API | ✅ done |
| 5 | Game engine (session state machine) | ✅ done |
| 6 | Live sessions over WebSockets, join API | ✅ done |
| 7 | UI: routing, home, admin dashboard | ✅ done |
| 8 | UI: quiz editor | ✅ done |
| 9 | UI: host slideshow and podium | ✅ done |
| 10 | UI: player (phone) view | ✅ done |
| 11 | README usage docs, end-to-end smoke test | ✅ done |
| 12 | Navigation: admin corner icon on home, Home and back-to-quizzes buttons | ✅ done |
| 13 | Question soundtracks: the host starts the music and the clock together | ✅ done |
| 14 | Open-ended questions: typed text and closest number (plan below) | 🚧 in progress |

## Plan: open-ended questions (step 14)

Two new question kinds next to today's multiple choice:

- **Typed text**: teams type a short answer (≤ 60 characters). It is right when it matches one of
  the accepted answers after normalising (case, accents, punctuation and spacing are ignored; no
  typo tolerance). On the reveal screen the answers are grouped by normalised text and the judge
  (host) can amend any group: ✓ full, ½ half or ✗ no points, plus a bonus or malus in steps of
  100 points (±1000 max). Points are the usual speed points × grade, plus the bonus, so a malus
  can push a team below zero (scores become signed).
- **Closest number**: teams enter a number; the closest guess (ties included) earns 1000 points,
  regardless of speed.

How it fits in:

- Model: `Question.kind` = `Choice` (default, so existing quizzes load unchanged) / `Text { accepted }`
  / `Number { answer }`; validation per kind.
- Protocol: `QuestionInfo.input` tells screens what to show; answers travel as a `Response`
  (choice indexes, text or number); `Reveal` becomes per kind (pick counts / text groups with
  judgements / guesses sorted by distance); `HostCommand::Judge` amends a text group.
- Engine: points are computed from stored responses and judgements when needed, so amendments
  update standings immediately; judging is possible whenever that question's answers are shown
  (after the question, or in the end-of-quiz review), and restarts the auto-advance pause.
- Phone: a text box or number field with Submit instead of coloured buttons.
- Big screen: "answer on your phone" while open; grouped answers with judge buttons, or the
  guesses around the true value, when revealed.
- Editor: kind picker per question; accepted answers list or the correct number.

## Notes

- `src/model.rs`: quizzes, questions, answers, settings and validation.
- `src/scoring.rs`: speed-weighted points, all-or-nothing/partial correctness, ranking.
- `src/server/store.rs`: one JSON file per quiz, atomic writes, path-safe ids.
- `src/server/media.rs`: picture re-encoding (questions → JPEG ≤1600×1200, avatars → 256² PNG), `GET /media/{name}`.
- `src/server/mod.rs`: global state initialised from `QUAESITO_DATA_DIR` (default `./data`).
- `src/server/auth.rs`: admin password from `QUAESITO_ADMIN_PASSWORD` (generated and logged if unset), in-memory bearer tokens.
- `src/api.rs`: server functions for login, quiz CRUD, duplicate, JSON import and picture upload.
- `src/protocol.rs`: host/player views and commands pushed over WebSockets (full view on every change).
- `src/server/engine.rs`: pure session state machine (lobby → questions → optional review → podium 3→2→1), time passed in explicitly for testability. Auto-advance pauses: 8 s after a revealed question, 3 s when hidden, 4 s per podium step.
- `src/server/sessions.rs`: live sessions by 6-digit code; a timer task per session fires deadlines and auto-advance.
- `src/server/live.rs`: WebSocket loops (`/api/ws/host`, `/api/ws/player`) pushing full views on every change.
- `src/server/net.rs`: join URL origin from `QUAESITO_PUBLIC_URL` or the LAN IP plus the host page's port (the QR code is drawn client-side, step 9).
- `src/ui/`: router (`/`, `/admin`, `/admin/quiz/:id`), `ClientOnly` wrapper so `localStorage`-dependent pages hydrate cleanly, admin gate with token check, dashboard (create/duplicate/export/import/delete), editor with ~1 s autosave and live validation.
- `GET /export/{id}?token=…` downloads a quiz as JSON.
- `assets/main.css`: single offline stylesheet (no web fonts/CDNs, the LAN may have no internet).
- `src/ui/live.rs`: `use_live` hook keeping a WebSocket alive with reconnection; countdowns run on the client from the last server snapshot.
- `src/ui/host.rs`: `/host/:code` big screen: lobby with client-side QR code, timed questions, reveals with pick counts, standings, podium 3→2→1. Advance with the button, →, PageDown or N (presentation clickers).
- Admin dashboard: start a quiz, list live games, reopen or end them.
- `src/ui/player.rs`: `/join` and `/join/:code` phone flow: team name, optional team picture (in the lobby), reconnect token per game in `localStorage`, tap-to-answer (single) or select-and-submit (multi), results, final rank.
- Soundtracks (step 13): `Question.audio`, `POST /api/admin/audio`, `Phase::Ready` in the engine (skipped for questions without music, never auto-advanced), `Soundtrack` component on the host footer without player controls (so the music can only start together with the clock) and an animated indicator while it plays; it starts on the host's "next" key/click, which also satisfies browser autoplay rules.
- End-to-end checked in headless Chrome (host + two phones) for both settings extremes: reveal after each/at end, manual/auto advance, all-or-nothing/partial, buttons-only/full text.

## Known gaps

- Pictures and soundtracks removed from a quiz stay on disk (no garbage collection yet).
- No automated browser tests in CI; the end-to-end flow was checked manually with headless Chrome.
- Admin tokens and live games live in memory: a server restart logs the admin out and ends games.
- Quiz JSON export does not include pictures or soundtracks.
- A team cannot be renamed or removed once joined.

## Ideas for later

- Sound effects (countdown, reveal, fanfare).
- Garbage-collect unused pictures; bundle pictures in exports.
- Show each team's answer time on the reveal screen.
