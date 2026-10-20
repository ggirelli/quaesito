# Troubleshooting

## Phones cannot connect

- Check that the phones are on the same Wi-Fi as the computer, and not on a guest network that
  keeps devices apart.
- Allow quaesito through the computer's firewall (Windows asks the first time).
- If the QR code shows the wrong address (VPNs, several network cards), set
  `QUAESITO_PUBLIC_URL`, for example `http://192.168.1.20:8080`; see
  [configuration](configuration.md).

## "Uploads only work on the computer quaesito runs on"

Pictures, soundtracks, videos and imports are only accepted on the computer running quaesito: open
the admin page there, at `http://localhost:8080`. On a server, use an SSH tunnel; see
[hosting on the internet](deployment.md#pictures-soundtracks-and-videos).

## I lost the admin password

Without `QUAESITO_ADMIN_PASSWORD`, a new password is generated at every start and shown in the
server window. Set the variable to choose your own.

## No sound on the big screen

Browsers only play sound after a click or key press on the page: click the big screen once.

## A phone shows "Reconnecting…"

It lost the connection (sleep, Wi-Fi). It reconnects by itself and keeps its team and answers;
reopening the join link also works.
