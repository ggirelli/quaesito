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
| 14 | Open-ended questions: typed text (judged) and closest number | ✅ done |
| 15 | Quiz list flags quizzes that are not ready to play and disables their Start button | ✅ done |
| 16 | Slides between questions: title, text and/or picture | ✅ done |
| 17 | Editor: shuffle questions within each run between slides, with undo | ✅ done |
| 18 | Points setting: speed-weighted or a flat 1000 per correct answer | ✅ done |
| 19 | Host renames and removes teams | ✅ done |
| 20 | Host pauses the clock, adds time and goes back | ✅ done |
| 21 | Reveal details: fastest team, answer times, rank changes | ✅ done |
| 22 | Joker: each team doubles one question's points | ✅ done |
| 23 | Picture answers for multiple choice | ✅ done |
| 24 | "Put in order" questions | ✅ done |
| 25 | Pictures revealed while the clock runs (zoom out or tile by tile; blur dropped: too hard to read until the end) | ✅ done |
| 26 | Sound effects on the big screen | ✅ done |
| 27 | End-of-game results page and download | ✅ done |
| 28 | Live games survive a server restart | ✅ done |
| 29 | Exports bundle media (zip); unused media is cleaned up | ✅ done |
| 30 | Fix: joining by typed code before the app has loaded (plain-form fallback to `/join?code=`) | ✅ done |
| 31 | Revealed pictures stay fully visible for a set number of seconds before the buzzer | ✅ done |
| 32 | Admin test mode: big screen and auto-joined phones side by side | ✅ done |
| 33 | Editor: move a question or slide to the top or bottom | ✅ done |
| 34 | Points multiplier per question (×1–×5) | ✅ done |
| 35 | True/false questions | ✅ done |
| 36 | Questions with several typed parts (title + artist, five posters…), judged per part | ✅ done |
| 37 | Pin on a picture (a map, a floor plan…), scored by distance | ✅ done |
| 38 | Icon buttons for edit, duplicate, export, import and delete (labels on hover) | ✅ done |
| 39 | Videos in place of question and slide pictures | ✅ done |
| 40 | Configurable upload limits, checked before uploading | ✅ done |
| 41 | Project boilerplate: MIT licence, contributing guide, code of conduct, security policy, templates, changelog | ✅ done |
| 42 | Documentation site (MkDocs, `just docs`) on GitHub Pages, with screenshots from `just screenshots` | ✅ done |
| 43 | Ready-to-run downloads for Linux, macOS (Apple silicon) and Windows, built by CI on version tags | ✅ done |
| 44 | Demo video with a Kokoro voice-over, captions and transcript (`just demo-video`) | ✅ done |
| 45 | Disclaimer: the app is fully vibe coded | ✅ done |

## Notes

- `src/model.rs`: quizzes, questions, answers, settings and validation.
- `src/scoring.rs`: speed-weighted points, all-or-nothing/partial correctness, judged typed answers, ranking.
- `src/server/matching.rs`: normalisation of typed answers (case, accents, punctuation, spacing).
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

- No automated browser tests in CI; `tools/browser` has headless-Chrome checks to run by hand.
- The repository is private: release downloads and private vulnerability reporting are only
  available to collaborators until it is made public (the documentation site is public).
- No download for Intel Macs yet (build from source there).
- `main` is protected by the "Protect main" ruleset: no deletion or force pushes, linear history,
  and pull requests with the CI (`test`, `clippy`, `fmt`, `deny`) and docs (`build`) checks
  passing. Repository admins can bypass it, so the maintainer can still push directly.

## Ideas for later

- Test mode: start the test at a chosen question.

- Team captains passing the phone: several players per team answering in turn.
