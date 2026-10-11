# Getting started

Quaesito runs on one computer, usually the laptop connected to the TV. Everyone else joins from
their phones over the same Wi-Fi; no internet connection is needed.

1. Download the archive for your system from the
   [latest release](https://github.com/ggirelli/quaesito/releases/latest) and unpack it anywhere.
2. Start it: double-click `start.bat` on Windows or `start.command` on macOS, or run `./start.sh`
   on Linux. A window opens with the server's log; leave it open while you play.
3. Your browser opens quaesito. Click the :material-cog: icon in the top right corner and log in
   with the admin password shown in the server window.

```text
────────────────────────────────────────────────────────────────────────
  quaesito is running

  On this computer:  http://localhost:8080
  Phones join at:    http://192.168.1.20:8080
  Admin password:    3f9c1a7b2e44   (set QUAESITO_ADMIN_PASSWORD to choose your own)
────────────────────────────────────────────────────────────────────────
```

!!! note "macOS and Windows"
    On macOS the first launch may be refused because the app is not signed: right-click
    `start.command`, choose **Open**, then **Open** again. Windows asks whether to let quaesito
    through the firewall: allow it on private networks so phones can connect.

The first time, your quiz list holds a **Demo quiz** with one question of each kind: press
**Test** to play it with pretend phones, or open it to see how it is made. Delete it when you no
longer need it; it does not come back.

This guide comes with the app too: the :material-help-circle-outline: icon on the home page and
**Help** in the admin pages open it, even without an internet connection.

Your quizzes, pictures and games are kept in the `data` folder next to the start script. Copy it
to back up or move everything to another computer.

Next: [build your first quiz](building-a-quiz.md).
