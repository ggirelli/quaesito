QUAESITO - quizzes among friends
================================

Start it
--------
  Windows: double-click start.bat
           (if "Windows protected your PC" appears, click More info, then Run anyway)
  macOS:   double-click start.command
           (the first time, macOS says it cannot verify it: click Done, open System Settings >
           Privacy & Security, click "Open Anyway" next to start.command, then double-click it
           again. On macOS 14 and earlier, right-click it, choose Open, then Open again.)
  Linux:   run ./start.sh in a terminal

A window opens with the server's log, and your browser opens quaesito. Leave the window open
while you play.

Log in as admin
---------------
Click the gear icon in the top right corner. The admin password is printed in the server window
("generated admin password: ..."). To choose your own, set QUAESITO_ADMIN_PASSWORD before
starting, for example on macOS or Linux:

  QUAESITO_ADMIN_PASSWORD=pick-something ./start.sh

Play
----
Try the Demo quiz first: press Test to play it with pretend phones. Then create a quiz (or import
one), press Test to try it, then Start. Show the big screen on a TV or
projector; teams scan its QR code with their phones, which must be on the same Wi-Fi.

If phones cannot connect, allow quaesito through the computer's firewall (Windows asks the first
time it starts). Your quizzes, pictures and games are kept in the "data" folder next to these
files: back it up, or copy it to move to another computer.

Help
----
The user guide is in the "docs" folder and opens from the app: click the question mark on the
home page, or Help in the admin pages. Online: https://ggirelli.github.io/quaesito/

More: https://github.com/ggirelli/quaesito
Licence: MIT (see LICENSE). Free and open source, provided as is, without warranty.
Fully vibe coded: an AI assistant wrote all of it.
Donations are welcome: https://ko-fi.com/ggirelli
