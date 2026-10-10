# Design

## Overview

Quaesito is a single Rust binary (Dioxus 0.7 fullstack, axum underneath) meant to run on a LAN.
It serves three views:

| View | Route | Device | Purpose |
|---|---|---|---|
| Admin | `/admin` | laptop | Password-protected quiz editor; start sessions |
| Host | `/host/<session>` | projector/TV | Slideshow: join QR code, lobby, questions, reveal, podium |
| Player | `/join/<code>` | phone | Team leader picks name + avatar, answers questions |

The server is authoritative for game state and timing. Clients receive state updates over
WebSockets; each running session is an in-memory state machine:

```
Lobby → Question(n) → Locked(n) → [Reveal(n)] → … → FinalReveal → Podium
```

## Quiz model

A quiz has an ordered list of questions. Each question has text, an optional picture, a time limit
(defaulting to the quiz-wide one) and two or more answers, any of which may be marked correct.

Per-quiz settings chosen by the admin:

| Setting | Options |
|---|---|
| Reveal | `AfterEachQuestion` / `AtEnd` |
| Multi-correct scoring | `AllOrNothing` / `Partial` |
| Advance | `Manual` (host clicks) / `Auto` (when time runs out or all teams answered) |
| Phone display | `ButtonsOnly` (read on the projector) / `FullText` |

## Scoring

A correct answer submitted after `t` seconds on a question with limit `T` earns
`round(1000 * (1 - 0.5 * t / T))` points: 1000 when instant, 500 at the buzzer.

With `Partial` scoring on multi-correct questions, the fraction
`max(0, (correct picked - wrong picked) / total correct)` multiplies the speed score.

Ties in the final ranking are broken by lower total response time on correct answers.

## Storage

- SQLite (`sqlx`) for quizzes, questions and answers.
- Uploaded images (question pictures, avatars) are re-encoded and resized server side and stored on
  disk under content-hashed names.
- Running sessions take a snapshot of the quiz at start, so editing never affects a live game.
- JSON export/import of quizzes for backup and sharing.

## Networking

Plain HTTP on the LAN. The join QR code points to the machine's LAN address, overridable via an
environment variable. Team leaders get a reconnect token stored in `localStorage`, so a sleeping
phone can rejoin without losing its score.
