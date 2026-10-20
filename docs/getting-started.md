# Getting started

Quaesito runs on one computer, usually the laptop connected to the TV. Everyone else joins from
their phones over the same Wi-Fi; no internet connection is needed.

## 1. Download and start it

Pick your system. Downloads are on the
[latest release](https://github.com/ggirelli/quaesito/releases/latest) page, under **Assets**.

=== "Windows"

    1. Download **`quaesito-…-windows-x86_64.zip`**.
    2. In your Downloads folder, right-click the zip and choose **Extract All…**, then
       **Extract**. Put the folder wherever you like, say on the Desktop.
    3. Open the folder and double-click **`start.bat`**.
    4. If a blue **"Windows protected your PC"** box appears (quaesito is not signed by a
       paying publisher), click **More info**, then **Run anyway**.
    5. When Windows asks whether to let quaesito through the firewall, tick **Private
       networks** and click **Allow**: this is what lets phones connect.

    A black window opens with the server's log, and your browser opens quaesito. Leave the window
    open while you play; close it to stop.

=== "macOS"

    For Macs with Apple silicon (M1 and later). On an Intel Mac,
    [build it from source](development.md).

    1. Download **`quaesito-…-macos-arm64.tar.gz`**. Safari unpacks it by itself; otherwise
       double-click it in your Downloads folder.
    2. Open the **`quaesito-…`** folder and double-click **`start.command`**.
    3. The first time, macOS says it **cannot verify** the app, because it is not signed by a
       paying developer. Click **Done** (or **Cancel**).
    4. Open **System Settings → Privacy & Security**, scroll down to the message about
       `start.command`, and click **Open Anyway**. Double-click `start.command` again and
       confirm with **Open Anyway** (and your password or Touch ID). On macOS 14 and earlier,
       right-clicking `start.command` and choosing **Open**, then **Open** again, does the same.
    5. If macOS asks whether to accept incoming network connections, click **Allow**: this is
       what lets phones connect.

    A Terminal window opens with the server's log, and your browser opens quaesito. Leave the
    window open while you play; close it to stop. Next time, a double-click is enough.

=== "Linux"

    For 64-bit PCs (x86-64).

    1. Download **`quaesito-…-linux-x86_64.tar.gz`**.
    2. Unpack it and start it from a terminal:

        ```sh
        tar xzf quaesito-*-linux-x86_64.tar.gz
        cd quaesito-*-linux-x86_64
        ./start.sh
        ```

    3. If your computer runs a firewall, let phones reach port 8080, for example with
       `sudo ufw allow 8080/tcp`.

    The terminal shows the server's log, and your browser opens quaesito. Leave it running while
    you play; press ++ctrl+c++ to stop.

The server window shows where quaesito is and the admin password:

```text
────────────────────────────────────────────────────────────────────────
  quaesito is running

  On this computer:  http://localhost:8080
  Phones join at:    http://192.168.1.20:8080
  Admin password:    3f9c1a7b2e44   (set QUAESITO_ADMIN_PASSWORD to choose your own)
────────────────────────────────────────────────────────────────────────
```

!!! tip "The browser did not open?"
    Open **<http://localhost:8080>** yourself, on the computer running quaesito.

## 2. Log in as admin

![The home page, with the admin icon in the top right corner](screenshots/home.webp)

Click the :material-cog: icon in the top right corner and type the admin password from the server
window. A new password is made at every start unless you choose your own (see
[configuration](configuration.md)). A login lasts 12 hours: the big screen uses it too, so log in
on the computer that runs it shortly before the game.

## 3. Try the demo quiz

![The admin page with the quiz list](screenshots/dashboard.webp)

Your quiz list starts with a **Demo quiz** holding one question of each kind. Press
**🧪 Test** to play it with pretend phones next to the big screen, or open it to see how it is
made. Delete it when you no longer need it; it does not come back.

## 4. Play with friends

Press **▶ Start** next to a quiz and put the big screen on the TV. Friends join by scanning its
QR code with phones **on the same Wi-Fi as the computer**: see [running the game](running-the-game.md).
If phones cannot connect, see [troubleshooting](troubleshooting.md#phones-cannot-connect).

This guide comes with the app too: the :material-help-circle-outline: icon on the home page and
**Help** in the admin pages open it, even without an internet connection.

Your quizzes, pictures and games are kept in the `data` folder next to the start script. Copy it
to back up or move everything to another computer.

Next: [build your first quiz](building-a-quiz.md).
