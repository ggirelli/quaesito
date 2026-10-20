# Status

Living tracker of the implementation plan. See [DESIGN.md](DESIGN.md) for the design itself.

## Plan

| #   | Step                                                                                                                               | Status  |
| --- | ---------------------------------------------------------------------------------------------------------------------------------- | ------- |
| 0   | Repo, CI, `just` recipes, cargo-deny, logo                                                                                         | ✅ done |
| 1   | Quiz model, per-quiz settings, validation                                                                                          | ✅ done |
| 2   | Scoring and ranking                                                                                                                | ✅ done |
| 3   | Storage (quizzes on disk) and image handling                                                                                       | ✅ done |
| 4   | Admin auth and quiz API                                                                                                            | ✅ done |
| 5   | Game engine (session state machine)                                                                                                | ✅ done |
| 6   | Live sessions over WebSockets, join API                                                                                            | ✅ done |
| 7   | UI: routing, home, admin dashboard                                                                                                 | ✅ done |
| 8   | UI: quiz editor                                                                                                                    | ✅ done |
| 9   | UI: host slideshow and podium                                                                                                      | ✅ done |
| 10  | UI: player (phone) view                                                                                                            | ✅ done |
| 11  | README usage docs, end-to-end smoke test                                                                                           | ✅ done |
| 12  | Navigation: admin corner icon on home, Home and back-to-quizzes buttons                                                            | ✅ done |
| 13  | Question soundtracks: the host starts the music and the clock together                                                             | ✅ done |
| 14  | Open-ended questions: typed text (judged) and closest number                                                                       | ✅ done |
| 15  | Quiz list flags quizzes that are not ready to play and disables their Start button                                                 | ✅ done |
| 16  | Slides between questions: title, text and/or picture                                                                               | ✅ done |
| 17  | Editor: shuffle questions within each run between slides, with undo                                                                | ✅ done |
| 18  | Points setting: speed-weighted or a flat 1000 per correct answer                                                                   | ✅ done |
| 19  | Host renames and removes teams                                                                                                     | ✅ done |
| 20  | Host pauses the clock, adds time and goes back                                                                                     | ✅ done |
| 21  | Reveal details: fastest team, answer times, rank changes                                                                           | ✅ done |
| 22  | Joker: each team doubles one question's points                                                                                     | ✅ done |
| 23  | Picture answers for multiple choice                                                                                                | ✅ done |
| 24  | "Put in order" questions                                                                                                           | ✅ done |
| 25  | Pictures revealed while the clock runs (zoom out or tile by tile; blur dropped: too hard to read until the end)                    | ✅ done |
| 26  | Sound effects on the big screen                                                                                                    | ✅ done |
| 27  | End-of-game results page and download                                                                                              | ✅ done |
| 28  | Live games survive a server restart                                                                                                | ✅ done |
| 29  | Exports bundle media (zip); unused media is cleaned up                                                                             | ✅ done |
| 30  | Fix: joining by typed code before the app has loaded (plain-form fallback to `/join?code=`)                                        | ✅ done |
| 31  | Revealed pictures stay fully visible for a set number of seconds before the buzzer                                                 | ✅ done |
| 32  | Admin test mode: big screen and auto-joined phones side by side                                                                    | ✅ done |
| 33  | Editor: move a question or slide to the top or bottom                                                                              | ✅ done |
| 34  | Points multiplier per question (×1–×5)                                                                                             | ✅ done |
| 35  | True/false questions                                                                                                               | ✅ done |
| 36  | Questions with several typed parts (title + artist, five posters…), judged per part                                                | ✅ done |
| 37  | Pin on a picture (a map, a floor plan…), scored by distance                                                                        | ✅ done |
| 38  | Icon buttons for edit, duplicate, export, import and delete (labels on hover)                                                      | ✅ done |
| 39  | Videos in place of question and slide pictures                                                                                     | ✅ done |
| 40  | Configurable upload limits, checked before uploading                                                                               | ✅ done |
| 41  | Project boilerplate: MIT licence, contributing guide, code of conduct, security policy, templates, changelog                       | ✅ done |
| 42  | Documentation site (MkDocs, `just docs`) on GitHub Pages, with screenshots from `just screenshots`                                 | ✅ done |
| 43  | Ready-to-run downloads for Linux, macOS (Apple silicon) and Windows, built by CI on version tags                                   | ✅ done |
| 44  | Demo video with a Chatterbox voice-over (Whisper-checked takes, Kokoro as a fallback), captions and transcript (`just demo-video`) | ✅ done |
| 45  | Disclaimer: the app is fully vibe coded                                                                                            | ✅ done |
| 46  | Demo quiz added on first start                                                                                                     | ✅ done |
| 47  | Help button and user guide bundled in the downloads (`/docs/`)                                                                     | ✅ done |
| 48  | Security review of the recent dependency changes (npm, cargo, docs, CI)                                                            | ✅ done |
| 49  | Docs home: "Made for game nights" right after the disclaimer, the podium screenshot stays at the bottom                            | ✅ done |
| 50  | Fully offline: no third-party requests from the app or the bundled guide (no Google Fonts, no GitHub API)                          | ✅ done |
| 51  | Editor: moving a question or slide keeps it in view and focused                                                                    | ✅ done |
| 52  | Results slides: show the standings mid-quiz (e.g. at an intermission) when revealing at the end                                    | ✅ done |
| 53  | Bonus: an open-ended follow-up on any closed question, worth a small extra (e.g. +50%)                                             | ✅ done |
| 54  | Text on question slides (lyrics, quotes…)                                                                                          | ✅ done |
| 55  | Document the team limit: 50 by default, any number of players per team; always capped for safety, but as high as you like          | ✅ done |
| 56  | Team limit set when starting a game: 50 by default, the host can lower or raise it                                                 | ✅ done |
| 57  | New colour palette and symbols in the assets (ASSETS.md prompts), docs and demo video                                              | ✅ done |
| 58  | Getting started: easier to follow, per platform                                                                                    | ✅ done |
| 59  | Closest number: optional range, with a slider next to the typed number on phones                                                   | ✅ done |
| 60  | Stopping the clock by hand stops the soundtrack                                                                                    | ✅ done |
| 61  | Reveals say so when no team answered                                                                                               | ✅ done |
| 62  | Admin quiz list: create button by Import (title in a pop-up), search box, sort by name, created, edited or questions               | ✅ done |
| 63  | Test mode: start the test from a chosen question or slide                                                                          | ✅ done |
| 64  | Release 0.2.0 (`v0.2.0` tag: downloads built by CI)                                                                                | ✅ done |
| 65  | Light theme, switched like the guide's (sun and moon button), kept per device                                                      | ✅ done |
| 66  | Release 0.3.0 (`v0.3.0` tag: downloads built by CI)                                                                                | ✅ done |
| 67  | Pairs questions: match each item on the left with its partner on the right                                                         | ✅ done |
| 68  | Versioned quiz file format with a published JSON Schema; import asks before leaving out unknown question kinds                     | ✅ done |
| 69  | Import and export progress bar; imported pictures processed in parallel                                                            | ✅ done |
| 70  | Release 0.4.0 (`v0.4.0` tag: downloads built by CI)                                                                                | ✅ done |
| 71  | Starting a game: option to turn off custom team pictures                                                                           | ✅ done |
| 72  | Custom team pictures with a transparent background, combined with a flat colour or gradient background of the team’s choice        | ✅ done |
| 73  | Question library: about 2,700 ready-made questions to browse by category, add while editing, or fill a new quiz at random          | ✅ done |
| 74  | Ready-made team pictures: 16 cut-out mascots to pick instead of uploading (also when uploads are off)                              | ✅ done |
| 75  | Phone lobby: “You’re in!” first, name and picture folded away (teams rename themselves); Teams panel toggles uploads               | ✅ done |
| 76  | Text, picture or video shown with the answer only (explanation, fun fact); quiz file format 2                                      | ✅ done |
| 77  | Soundtrack played with the answer (next to its picture or video)                                                                   | ✅ done |
| 78  | Questions filed under the library’s categories and topics, shown on the big screen; topics added from the admin page               | ✅ done |
| 79  | Editor: question navigator, scroll position in the URL, table view; floating field labels and boxed media                          | ✅ done |
| 80  | Library: report a wrong question in a GitHub issue; save quiz questions to it and keep them in step; Source filter                 | ✅ done |
| 81  | Wi-Fi slide with a QR code before the join code                                                                                    | ✅ done |
| 82  | Big screen: picture beside a slide’s text or a question’s passage; cog button back to the admin page                               | ✅ done |
| 83  | Release 0.5.0 (`v0.5.0` tag: downloads built by CI)                                                                                | ✅ done |
| 84  | Hosting on the internet: deployment guide, Linux ARM64 downloads, guessing throttle, local-only uploads, HTTPS join links          | ✅ done |
| 85  | Big screen: **New teams can join** toggle; every slide and the podium fit any screen size                                          | ✅ done |
| 86  | ↺ Reopen a question closed by mistake                                                                                              | ✅ done |
| 87  | Release 0.6.0 (`v0.6.0` tag: downloads built by CI)                                                                                | ✅ done |

## Notes

- `src/model.rs`: quizzes, questions, answers, settings and validation.
- `src/scoring.rs`: speed-weighted points, all-or-nothing/partial correctness, judged typed answers, ranking.
- `src/server/matching.rs`: normalisation of typed answers (case, accents, punctuation, spacing).
- `src/server/store.rs`: one JSON file per quiz, atomic writes, path-safe ids.
- `src/server/media.rs`: picture re-encoding (questions → JPEG ≤1600×1200, avatars → 256² PNG keeping transparency), `GET /media/{name}`.
- Team pictures: an upload (unless the game turns uploads off) or one of 16 ready-made mascots (`assets/teams/`, made by `tools/assets/team-mascots.py`), on a background the team picks (`.backdrop-<n>` classes; inline styles flickered). Mascots were drawn for
  the sky-blue background; on others a `hue-rotate()` per background class keeps the same contrast.
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
- Offline (step 50): the user guide self-hosts its fonts (Material's privacy plugin) and shows the repository link without
  GitHub's facts (`overrides/partials/source.html`). `RELEASE=1 DEMO_QUIZ=1 tools/browser/run.sh check-offline.js` fails on
  any request outside the server. The server sends a content security policy allowing only itself, which also blocks the
  Inter font that dx's rebuild notice imports from Google in debug builds.
- `src/ui/live.rs`: `use_live` hook keeping a WebSocket alive with reconnection; countdowns run on the client from the last server snapshot.
- `src/ui/host.rs`: `/host/:code` big screen: lobby with client-side QR code, timed questions, reveals with pick counts, standings, podium 3→2→1. Advance with the button, →, PageDown or N (presentation clickers).
- Admin dashboard: start a quiz, list live games, reopen or end them.
- `src/ui/player.rs`: `/join` and `/join/:code` phone flow: team name, optional team picture and background (folded away in the lobby), reconnect token per game in `localStorage`, tap-to-answer (single) or select-and-submit (multi), results, final rank.
- Soundtracks (step 13): `Question.audio`, `POST /api/admin/audio`, `Phase::Ready` in the engine (skipped for questions without music, never auto-advanced), `Soundtrack` component on the host footer without player controls (so the music can only start together with the clock) and an animated indicator while it plays; it starts on the host's "next" key/click, which also satisfies browser autoplay rules.
- End-to-end checked in headless Chrome (host + two phones) for both settings extremes: reveal after each/at end, manual/auto advance, all-or-nothing/partial, buttons-only/full text.

## Known gaps

- No automated browser tests in CI; `tools/browser` has headless-Chrome checks to run by hand.
- The repository is private: release downloads and private vulnerability reporting are only
  available to collaborators until it is made public (the documentation site is public).
- No download for Intel Macs yet (build from source there).
- `main` is protected by the "Protect main" ruleset: no deletion or force pushes, linear history,
  and pull requests with the CI (`test`, `clippy`, `fmt`, `deny`) and docs (`build`) checks
  passing. Nobody bypasses it: every change, the maintainer's too, goes through a pull request.

## Ideas for later

- Team captains passing the phone: several players per team answering in turn.
