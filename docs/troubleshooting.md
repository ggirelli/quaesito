# Troubleshooting

Most problems show up in the **server window**: the window that opened when you started
quaesito (on a server, `journalctl -u quaesito`). It logs what quaesito does, and every error.
Keep it open, and look there first.

## Quaesito does not start

**Symptoms:** the server window closes straight away, or shows an error instead of
"quaesito is running".

- **Windows says "Windows protected your PC"**, or **macOS says it cannot verify the app:**
  quaesito is not signed by a paying publisher. Follow the steps for your system in
  [getting started](getting-started.md#1-download-and-start-it).
- **The log says `listening on 0.0.0.0:8080: …`** and "Another program, perhaps another
  quaesito window, already uses port 8080": something else already uses port 8080, often
  quaesito itself, started earlier in another window. Close the other one, or start this one on
  another port:

    === "macOS and Linux"

        ```sh
        PORT=8081 ./start.sh
        ```

        (`start.command` reads `PORT` the same way when started from a terminal.)

    === "Windows"

        In a Command Prompt, in quaesito's folder:

        ```bat
        set PORT=8081
        start.bat
        ```

    Then open `http://localhost:8081`.

- **The window closes before you can read the error** (macOS and Linux): start it from a
  terminal, in quaesito's folder (`./start.command` on macOS, `./start.sh` on Linux), to keep the
  message on screen.
- **Linux says it cannot find `GLIBC_2.…`:** the downloads need glibc 2.35 or later (Ubuntu
  22.04 or newer, or an equally recent distribution). On an older system,
  [build it from source](development.md#running-it-from-source).

If the log shows another error, [report a bug](#reporting-a-problem) with it.

## The admin page does not open

**Symptoms:** the browser did not open, or shows "unable to connect" at `http://localhost:8080`.

1. Check that the server window is still open and says "quaesito is running". If not, see
   [above](#quaesito-does-not-start).
2. Open the address the server window gives **on this computer**, usually
   `http://localhost:8080`, in the browser of the computer running quaesito. If you chose
   another `PORT`, use that.
3. Reload the page once the server is running: the browser may have opened before the server
   was ready.

## Phones cannot connect

**Symptoms:** the QR code or join address does not load on phones, or loads only on the
computer itself.

Check, in this order:

1. **Same network:** the phones must be on the same Wi-Fi as the computer, and not on mobile
   data (some phones switch to it when the Wi-Fi is slow). **Guest networks** and some venue or
   office networks keep devices apart: they cannot reach each other at all. Use a home network,
   or a phone's hotspot that the computer joins too.
2. **The firewall:** quaesito must be allowed to accept connections.
    - **Windows:** if you missed the question at the first start, open **Windows Security →
      Firewall & network protection → Allow an app through firewall**, and allow `server`
      (quaesito's server). Check that it is allowed for the kind of network you are on: Windows
      treats a network set as **Public** differently from a **Private** one.
    - **macOS:** if you clicked **Don't Allow**, open **System Settings → Network → Firewall →
      Options**, and allow incoming connections for `server`.
    - **Linux:** let phones reach the port, for example `sudo ufw allow 8080/tcp`.
3. **The address:** compare the address on the big screen with **Phones join at** in the server
   window. Open it in a browser on a phone, or on another computer on the same Wi-Fi. If it
   shows the wrong address (a VPN, several network connections, a virtual machine), set
   `QUAESITO_PUBLIC_URL` to the computer's address on the Wi-Fi, for example
   `http://192.168.1.20:8080`; see [configuration](configuration.md).
4. **The server window says phones are "not reachable":** quaesito was started listening on this
   computer only (`IP=127.0.0.1`). Start it with the start script, which listens on the network,
   or set `IP=0.0.0.0`.

If the address loads on another computer but not on the phones, the phones are on another
network: go back to step 1.

## "Uploads only work on the computer quaesito runs on"

Pictures, soundtracks, videos and imports are only accepted on the computer running quaesito,
with the admin page opened at `http://localhost:8080` (or `127.0.0.1`). Another computer, a
tablet or a phone cannot upload, even on the same Wi-Fi, and neither can the computer itself if
its admin page is opened at its network address (`http://192.168.…`): then the message links to
the same page at `localhost`. On a server, use an SSH tunnel; see
[hosting on the internet](deployment.md#pictures-soundtracks-and-videos).

"Team pictures can only be uploaded from the network the quiz runs on" means the phone is not
on the computer's network (on mobile data, say): it can pick a ready-made picture instead.

## I lost the admin password

Without `QUAESITO_ADMIN_PASSWORD`, a new password is generated at every start and shown in the
server window (look for "Admin password", or "generated admin password" in the log). Set the
variable to choose your own; see [configuration](configuration.md).

"Too many wrong passwords: try again in a few minutes" means 10 wrong passwords were typed from
this device within five minutes: wait five minutes, then try again.

## A quiz is missing or does not load

- **A quiz is flagged in the list:** it is not ready to play yet. Open it: the editor says what
  is missing, such as a multiple-choice question without a right answer.
- **A quiz file was there but does not show:** quaesito skips quiz files it cannot read, and the
  server window says so each time the quiz list loads: `skipping …/quizzes/<file>.json: …`, with
  the reason. This
  happens to files edited by hand or copied in half. Fix the file (see the
  [quiz file format](quiz-file-format.md)), or restore it from a backup.
- **Importing asks about a newer version:** the quiz was made with a newer Quaesito; see
  [versions](quiz-file-format.md#versions). Update Quaesito to keep everything.

## My quizzes are gone

**Symptoms:** the quiz list is empty, or shows only the Demo quiz again.

Quaesito keeps everything in its **data folder**, and the most likely cause is that this start
used a different one. When it starts, the server window says which: "Your quizzes and pictures
are kept in …" and `data directory: …`.

- **After unpacking a new version** into a new folder, the new folder has an empty `data` folder
  of its own. Stop quaesito, copy the `data` folder from the old version's folder into the new
  one (replacing the new, empty one), and start it again. See
  [updating](getting-started.md#your-data).
- **After moving the folder, or starting the server some other way:** without the start script,
  the data folder is `data` in the folder the server was started from. Start it with the start
  script, or set `QUAESITO_DATA_DIR` to the folder that holds your quizzes.

If the right folder is in use and still looks empty, check that its `quizzes` folder holds your
`.json` files, and look for `skipping …` lines in the log ([above](#a-quiz-is-missing-or-does-not-load)).

## Backing up and restoring

Everything quaesito keeps is in the data folder: see [the data folder](configuration.md#the-data-folder).

1. **Back up:** stop quaesito (close its window), then copy the whole `data` folder somewhere
   safe. To back up a single quiz with its pictures, use **Export** on the admin page instead.
2. **Restore:** stop quaesito, replace the `data` folder with your copy, and start it again.

Keep backups private: `admin-tokens.json` holds the logins of admin browsers. On a server, see
[backups](deployment.md#backups).

## No sound on the big screen

Browsers only play sound after a click or key press on the page: click the big screen once.
Check that sound effects are not muted (:material-volume-high: in the top bar), and that the
computer's sound goes to the TV.

## A phone shows "Reconnecting…"

It lost the connection (sleep, Wi-Fi). It reconnects by itself and keeps its team and answers;
reopening the join link also works. If it never reconnects, check that the server is still
running, then see [phones cannot connect](#phones-cannot-connect).

## Reporting a problem

If none of this helps, [open an issue](https://github.com/ggirelli/quaesito/issues/new/choose):

- **Bug report:** what happened, the steps that led to it, your system and browser, and the
  lines of the server window's log around the problem. Remove the admin password from them.
- **Wrong library question:** use the question's **report** link in the app, which fills it in
  for you (see [the question library](question-library.md#fixing-a-question)).
- **Security problems:** never in a public issue; see the
  [security policy](https://github.com/ggirelli/quaesito/blob/main/SECURITY.md).
