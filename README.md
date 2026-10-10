<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo-dark.png">
    <img src="assets/brand/logo-light.png" alt="quaesito" width="480">
  </picture>
</h1>

Quizzes among friends: a self-hosted, live quiz web app written in Rust with [Dioxus](https://dioxuslabs.com).

- **Admin** builds quizzes in the browser: questions, optional pictures and soundtracks, 2–6 answers (one or several correct) and time limits.
- **Big screen** shows the quiz as a slideshow on a TV or projector, with a QR code to join.
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

A question can have a soundtrack (MP3, M4A/AAC, Ogg/Opus, WebM, FLAC or WAV, up to 40 MB). It then
waits on the big screen with a full clock until the host presses **▶ Start the music**: the music
plays from the big screen's speakers and the countdown starts with it. This also holds with
automatic advance. Phones show "Listen up!" until then.

If phones cannot connect, allow incoming connections on the port in the laptop's firewall. If the detected address is
wrong (VPNs, several network cards), set `QUAESITO_PUBLIC_URL`, e.g. `http://192.168.1.20:8080`.

### Quiz settings

Each quiz chooses:

| Setting | Options |
|---|---|
| Reveal answers | after each question (with standings) · at the end (walk through the answers before the podium) |
| Several correct answers | all or nothing · partial credit (correct picks minus wrong picks) |
| Moving on | host clicks · automatic after a short pause |
| On team phones | coloured buttons only (read on the big screen) · full question and answer text |

A correct answer earns 1000 points when instant, down to 500 at the buzzer. Ties are broken by the time spent on
correct answers.

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
