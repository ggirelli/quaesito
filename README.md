<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo-dark.png">
    <img src="assets/brand/logo-light.png" alt="quaesito" width="480">
  </picture>
</h1>

Quizzes among friends: a self-hosted, Kahoot-style quiz web app written in Rust with [Dioxus](https://dioxuslabs.com).

- An admin builds quizzes (questions, optional pictures, answers, time limits) in the built-in editor.
- The host screen renders the quiz as a slideshow on a projector/TV.
- Team leaders join from their phones via QR code or link plus a session code, pick a name and an avatar.
- Faster correct answers earn more points; the podium is revealed 3rd → 2nd → 1st.

Designed to run on a LAN.

## Development

Requires Rust (stable), the `wasm32-unknown-unknown` target, [`dx`](https://dioxuslabs.com/learn/0.7/getting_started) (Dioxus CLI 0.7), [`just`](https://github.com/casey/just) and [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny).

```sh
just serve   # run the app with hot reload
just check   # fmt + clippy + tests + cargo-deny, same as CI
```
