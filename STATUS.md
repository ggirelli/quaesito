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
| 7 | UI: routing, home, admin quiz list | ⏳ next |
| 8 | UI: quiz editor | ⬜ todo |
| 9 | UI: host slideshow and podium | ⬜ todo |
| 10 | UI: player (phone) view | ⬜ todo |
| 11 | README usage docs, end-to-end smoke test | ⬜ todo |

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
