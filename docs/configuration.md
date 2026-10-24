# Configuration

Quaesito reads a few environment variables: settings given to a program when it starts. The start
scripts set sensible defaults; set the variables before starting to change them:

=== "macOS and Linux"

    ```sh
    QUAESITO_ADMIN_PASSWORD=pick-something ./start.sh
    ```

    (or `./start.command` on macOS, from a terminal in quaesito's folder)

=== "Windows"

    In a Command Prompt, in quaesito's folder:

    ```bat
    set QUAESITO_ADMIN_PASSWORD=pick-something
    start.bat
    ```

| Variable | Default | Meaning |
|---|---|---|
| `QUAESITO_ADMIN_PASSWORD` | generated at every start | the admin password |
| `QUAESITO_DATA_DIR` | `data` next to the start script | where quizzes, pictures, soundtracks and games are kept |
| `QUAESITO_PUBLIC_URL` | this computer's LAN address, or the big screen's address if it was opened over HTTPS | the address used in join links and the QR code, such as `https://quiz.example.com` |
| `IP` / `PORT` | `0.0.0.0` / `8080` | where the server listens (`0.0.0.0` accepts phones on the network; `127.0.0.1` only this computer, as behind a reverse proxy) |
| `QUAESITO_DOCS_DIR` | `docs` next to the start script | the user guide served at `/docs/`; without one, `/docs/` leads to the online guide |
| `QUAESITO_MAX_PICTURE_MB` | `15` | largest picture upload |
| `QUAESITO_MAX_AUDIO_MB` | `40` | largest soundtrack upload |
| `QUAESITO_MAX_VIDEO_MB` | `200` | largest video upload |
| `QUAESITO_MAX_IMPORT_MB` | `500` | largest quiz bundle to import (media included) |

The defaults above are the start scripts'. The server program on its own (`server`, or
`server.exe` on Windows, started without a start script) listens on `127.0.0.1` only, and keeps
its data in `data` in the folder it was started from.

To run quaesito on a server that players reach over the internet, see
[hosting on the internet](deployment.md).

## The data folder

| Path | Contents |
|---|---|
| `quizzes/` | one JSON file per quiz |
| `media/` | uploaded pictures, soundtracks and videos; unused ones are removed after a day |
| `games/` | live games, so they survive a restart |
| `admin-tokens.json` | admin logins, so you stay logged in across restarts; each lasts 12 hours |
| `library-edits.json` | your edits to questions of the [question library](question-library.md#fixing-a-question) |
| `library-added.json` | questions [saved to the library](question-library.md#saving-questions-to-the-library) from your quizzes |
| `library-topics.json` | categories and topics [you added](building-a-quiz.md#category-and-topic) |
| `demo-quiz-added` | a marker that the Demo quiz was added once; delete it to have the Demo quiz added again (only into an empty quiz list) |

Copy the whole folder to back everything up or move it to another computer; see
[backing up and restoring](troubleshooting.md#backing-up-and-restoring).
