QUAESITO - quizzes among friends
================================

Start it
--------
  Windows: double-click start.bat
  macOS:   double-click start.command
           (the first time, macOS may refuse to open it: right-click it, choose Open, then Open
           again. If it is still blocked, open System Settings > Privacy & Security and click
           "Open Anyway".)
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
Licence: MIT (see LICENSE)
