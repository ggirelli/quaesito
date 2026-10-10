# Status

Living tracker of the implementation plan. See [DESIGN.md](DESIGN.md) for the design itself.

## Plan

| # | Step | Status |
|---|---|---|
| 0 | Repo, CI, `just` recipes, cargo-deny, logo | ✅ done |
| 1 | Quiz model, per-quiz settings, validation | ✅ done |
| 2 | Scoring and ranking | ✅ done |
| 3 | Storage (quizzes on disk) and image handling | ✅ done |
| 4 | Admin auth and quiz API | ⏳ next |
| 5 | Game engine (session state machine) | ⬜ todo |
| 6 | Live sessions over WebSockets, join API, QR code | ⬜ todo |
| 7 | UI: routing, home, admin quiz list | ⬜ todo |
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
