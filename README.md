<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo-dark.png">
    <img src="assets/brand/logo-light.png" alt="quaesito" width="480">
  </picture>
</h1>

<p align="center">📖 <a href="https://ggirelli.github.io/quaesito/"><b>Documentation, with screenshots</b></a></p>

**Quizzes among friends.** Put the questions on the TV and let every team answer from a phone. Quaesito runs on
your laptop over the local Wi-Fi: no internet, no accounts, no app to install.

> [!NOTE]
> Quaesito is **fully vibe coded**: an AI assistant (Claude) wrote all of its code, tests and docs. Read the
> [disclaimer](DISCLAIMER.md) before relying on it.

## How it works

1. **Build** a quiz in the browser: questions with pictures, videos or soundtracks, and slides between rounds.
   **Test** it with pretend phones before the night.
2. **Show** the big screen on a TV or projector. Teams join by scanning its QR code.
3. **Play**: teams answer against the clock, the big screen reveals answers and standings, and a podium crowns the
   winner. Results can be printed or downloaded as CSV.

**Question types:** multiple choice, true or false, typed answers (judged by the host on the big screen), closest
number, put in order, several answers in one, and pin on a picture. Each quiz sets its own scoring, reveals and pace.
See [question types](https://ggirelli.github.io/quaesito/question-types/) and [quiz settings](https://ggirelli.github.io/quaesito/quiz-settings/).

## Get started

1. Download the archive for your system (Windows, macOS on Apple silicon, Linux) from the
   [latest release](https://github.com/ggirelli/quaesito/releases/latest) and unpack it.
2. Double-click `start.bat` (Windows) or `start.command` (macOS), or run `./start.sh` (Linux).
3. Your browser opens quaesito. Log in with the admin password shown in the server window, and try the **Demo quiz**.

The [getting started guide](https://ggirelli.github.io/quaesito/getting-started/) covers the details, and the user guide also ships with the app
(**Help** in the admin pages).

## Development

Requires Rust (stable) with the `wasm32-unknown-unknown` target, [`dx`](https://dioxuslabs.com/learn/0.7/getting_started)
(Dioxus CLI 0.7), [`just`](https://github.com/casey/just) and [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny).

```sh
just serve   # hot-reloading dev server (admin password "quaesito")
just play    # release build served on the LAN, for real games
just check   # fmt, clippy, tests and cargo-deny, as in CI
just docs    # the user guide, live at http://127.0.0.1:8000
```

More in the [development guide](https://ggirelli.github.io/quaesito/development/), [DESIGN.md](planning/DESIGN.md) and
[STATUS.md](planning/STATUS.md).

## Contributing

Bug reports, ideas and pull requests are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md) and the
[code of conduct](CODE_OF_CONDUCT.md). Report security problems privately as described in
[SECURITY.md](SECURITY.md). Changes are listed in [CHANGELOG.md](CHANGELOG.md).

## Licence

[MIT](LICENSE)
