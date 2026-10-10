# Status

Living tracker of the implementation plan. See [DESIGN.md](DESIGN.md) for the design itself.

## Plan

| # | Step | Status |
|---|---|---|
| 0 | Repo, CI, `just` recipes, cargo-deny, logo | ✅ done |
| 1 | Quiz model, per-quiz settings, validation | ✅ done |
| 2 | Scoring and ranking | ⏳ next |
| 3 | Storage (quizzes on disk) and image handling | ⬜ todo |
| 4 | Admin auth and quiz API | ⬜ todo |
| 5 | Game engine (session state machine) | ⬜ todo |
| 6 | Live sessions over WebSockets, join API, QR code | ⬜ todo |
| 7 | UI: routing, home, admin quiz list | ⬜ todo |
| 8 | UI: quiz editor | ⬜ todo |
| 9 | UI: host slideshow and podium | ⬜ todo |
| 10 | UI: player (phone) view | ⬜ todo |
| 11 | README usage docs, end-to-end smoke test | ⬜ todo |

## Notes

- `src/model.rs`: quizzes, questions, answers, settings and validation.
