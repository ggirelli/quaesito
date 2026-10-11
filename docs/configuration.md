# Configuration

Quaesito reads a few environment variables. The start scripts set sensible defaults; set the
variables before starting to change them, for example on macOS or Linux:

```sh
QUAESITO_ADMIN_PASSWORD=pick-something ./start.sh
```

| Variable | Default | Meaning |
|---|---|---|
| `QUAESITO_ADMIN_PASSWORD` | generated at every start | the admin password |
| `QUAESITO_DATA_DIR` | `data` next to the start script | where quizzes, pictures, soundtracks and games are kept |
| `QUAESITO_PUBLIC_URL` | this computer's LAN address | the address used in join links and the QR code |
| `IP` / `PORT` | `0.0.0.0` / `8080` | where the server listens (`0.0.0.0` accepts phones on the network) |
| `QUAESITO_DOCS_DIR` | `docs` next to the start script | the user guide served at `/docs/`; without one, `/docs/` leads to the online guide |
| `QUAESITO_MAX_PICTURE_MB` | `15` | largest picture upload |
| `QUAESITO_MAX_AUDIO_MB` | `40` | largest soundtrack upload |
| `QUAESITO_MAX_VIDEO_MB` | `200` | largest video upload |
| `QUAESITO_MAX_IMPORT_MB` | `500` | largest quiz bundle to import (media included) |

## The data folder

| Path | Contents |
|---|---|
| `quizzes/` | one JSON file per quiz |
| `media/` | uploaded pictures, soundtracks and videos; unused ones are removed after a day |
| `games/` | live games, so they survive a restart |
| `admin-tokens.json` | admin logins, so you stay logged in across restarts |
