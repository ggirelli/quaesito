# Hosting on the internet

!!! danger "At your own risk"
    **Putting quaesito on the internet can be dangerous, and you do it entirely at your own risk.**

    Quaesito was made for a trusted local network. Its code has **not been fully tested or audited** for
    security, and it was written by an AI assistant. Once it is reachable from the internet, anyone can try
    to break into it, the server it runs on, or the data you put on it, and it may have weaknesses nobody
    has found yet.

    Quaesito is provided **as is, without warranty of any kind**, under the
    [MIT licence](https://github.com/ggirelli/quaesito/blob/main/LICENSE). Its authors and contributors are
    **not liable** for any damage, data loss, costs, or security incidents that result from running it, on a
    public server or anywhere else. See the
    [disclaimer](https://github.com/ggirelli/quaesito/blob/main/DISCLAIMER.md).

    If you go ahead: keep nothing private or sensitive on the server, use a strong admin password, keep the
    server and quaesito updated, and switch it off when you are not playing.

Quaesito is made for a laptop on the local Wi-Fi, and that stays the simplest way to play. To let teams
join from anywhere instead (remote players, a venue without usable Wi-Fi), run it on a small Linux
server: players still only need a browser.

This page sets up one server for one host, as on the laptop: one admin password, quizzes and pictures in
a data folder. It uses [Caddy](https://caddyserver.com/) for HTTPS and systemd to keep quaesito running.

```text
players' browsers ──HTTPS──▶ Caddy (ports 80, 443) ──HTTP and WebSockets──▶ quaesito (127.0.0.1:8080)
                                                                                  │
                                                                     /var/lib/quaesito (data)
```

## What you need

- A Linux server with a public IP address: x86-64 or ARM64 (64-bit), with systemd. The steps use
  Ubuntu 22.04 or later. Quaesito needs little: 1 CPU and 1 GB of memory are enough for a quiz night,
  plus disk space for your pictures and videos.
- A **hostname** pointing at the server's IP address, such as `quiz.example.com`: HTTPS certificates
  are issued for names, so a stable hostname is needed for a dependable setup. A subdomain of a domain
  you own works, as does a free dynamic DNS name. For a first try, `<ip-with-dashes>.sslip.io` (for
  example `203-0-113-7.sslip.io`) resolves to that IP with no sign-up, but it is shared by many people
  and certificates may be refused when too many were issued.
- Ports 80 and 443 open to the internet (port 80 is needed to get the certificate). Keep port 8080
  closed: quaesito listens on `127.0.0.1` only, behind Caddy.

!!! warning "Before going public"
    - Choose a **long random admin password** (`openssl rand -base64 24`). Anyone on the internet can
      reach the login page.
    - Run quaesito behind Caddy, on the same machine, with quaesito listening on `127.0.0.1` only.
      Quaesito trusts the `X-Forwarded-For` header of a proxy on its own machine to tell visitors
      apart, and Caddy sets it. With another proxy, make sure it sets that header too (in nginx:
      `proxy_set_header X-Forwarded-For $remote_addr;`): otherwise visitors may look like the server
      itself, which may upload files.

## Install

Run these on the server.

1. **Download** the Linux archive for the server's processor (`uname -m`: `aarch64` is
   `linux-arm64`, `x86_64` is `linux-x86_64`) from the
   [latest release](https://github.com/ggirelli/quaesito/releases/latest), and unpack it in
   `/opt/quaesito`:

    ```sh
    curl -fLO https://github.com/ggirelli/quaesito/releases/download/vX.Y.Z/quaesito-X.Y.Z-linux-arm64.tar.gz
    sudo mkdir -p /opt/quaesito
    sudo tar -xzf quaesito-X.Y.Z-linux-arm64.tar.gz -C /opt/quaesito --strip-components=1
    ```

    To build it yourself instead, see [building from source](#building-from-source).

2. **Create the user** quaesito runs as. It cannot log in, and owns nothing but the data folder:

    ```sh
    sudo useradd --system --no-create-home --home-dir /var/lib/quaesito --shell /usr/sbin/nologin quaesito
    ```

3. **Configure it.** Copy the example settings, then set your hostname and password:

    ```sh
    sudo mkdir -p /etc/quaesito
    sudo install -m 600 /opt/quaesito/deploy/quaesito.env.example /etc/quaesito/quaesito.env
    sudo nano /etc/quaesito/quaesito.env
    ```

    | Variable | Set it to |
    |---|---|
    | `QUAESITO_PUBLIC_URL` | `https://` and your hostname, e.g. `https://quiz.example.com`: join links and the QR code use it |
    | `QUAESITO_ADMIN_PASSWORD` | a long random password |

    The service sets `IP=127.0.0.1`, `PORT=8080` and `QUAESITO_DATA_DIR=/var/lib/quaesito`; the
    [configuration](configuration.md) page lists every setting.

4. **Start the service**, now and at every boot:

    ```sh
    sudo cp /opt/quaesito/deploy/quaesito.service /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable --now quaesito
    systemctl status quaesito
    ```

5. **Install Caddy** ([other systems](https://caddyserver.com/docs/install)):

    ```sh
    sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
    sudo apt update && sudo apt install -y caddy
    ```

6. **Point Caddy at quaesito.** Copy the example and replace `quiz.example.com` with your hostname:

    ```sh
    sudo cp /opt/quaesito/deploy/Caddyfile /etc/caddy/Caddyfile
    sudo nano /etc/caddy/Caddyfile
    sudo systemctl reload caddy
    ```

    Caddy gets the certificate within a minute (`journalctl -u caddy` shows how it goes), renews it,
    and redirects `http://` to `https://`. WebSockets need no extra setting.

7. **Open the firewall** for ports 80 and 443: in your cloud provider's settings, and on the server
   if it runs one (`sudo ufw allow 80,443/tcp` with ufw). See [Oracle Cloud](#oracle-cloud) for its
   two firewalls.

Then open `https://quiz.example.com/admin` and log in.

## Pictures, soundtracks and videos

Nothing can be uploaded to quaesito over the internet: question pictures, soundtracks, videos and
quiz imports are only accepted from the server itself. Team pictures are only accepted from phones
on the server's local network, so players joining over the internet pick one of the ready-made
pictures instead.

To add media to a quiz on the server, open the admin page through an **SSH tunnel**, which makes
your browser's requests come from the server. On your computer:

```sh
ssh -N -L 8080:127.0.0.1:8080 ubuntu@quiz.example.com
```

Leave it running, open `http://localhost:8080/admin`, and log in again (logins are kept per
address). Edit, upload or import as usual, then close the tunnel with ++ctrl+c++. Everything else
(starting games, the big screen) works at the public address.

You can also build the quiz on your own computer, export it, and import the `.zip` through the
tunnel.

## Running it

| To | Run |
|---|---|
| see whether it runs | `systemctl status quaesito` |
| read its log | `journalctl -u quaesito -f` (`-b` for since boot) |
| restart it | `sudo systemctl restart quaesito` |
| stop it | `sudo systemctl stop quaesito` |
| change settings | edit `/etc/quaesito/quaesito.env`, then restart |

Live games are saved as they go: after a restart or reboot they come back paused, the big screen and
phones reconnect by themselves, and the host resumes. Only the last fraction of a second before a stop
can be lost.

**Updating:** unpack the new archive over `/opt/quaesito`, then restart. The data folder is not
touched. Check the [changelog](https://github.com/ggirelli/quaesito/blob/main/CHANGELOG.md) for notes
first.

## Backups

Everything quaesito keeps is in `/var/lib/quaesito`: see [the data folder](configuration.md#the-data-folder).
Files are written whole, so a copy taken while it runs is usable; stop it for a copy of a game in progress.

```sh
# Back up
sudo tar -czf quaesito-$(date +%F).tar.gz -C /var/lib quaesito

# Restore
sudo systemctl stop quaesito
sudo rm -rf /var/lib/quaesito
sudo tar -xzf quaesito-2026-10-20.tar.gz -C /var/lib
sudo chown -R quaesito:quaesito /var/lib/quaesito
sudo systemctl start quaesito
```

Keep backups private: `admin-tokens.json` holds the logins of admin browsers. Exporting a quiz from the
admin page (**Export**) also saves it with its media, to import on any quaesito.

## Checking the setup

- [ ] `https://` loads the home page with a valid certificate, and `http://` redirects to it.
- [ ] Port 8080 is closed from outside: `curl -m 5 http://quiz.example.com:8080` fails.
- [ ] The admin page logs in with your password; wrong passwords are refused.
- [ ] Start the Demo quiz: the big screen shows `https://quiz.example.com/join/…` and the QR code opens it.
- [ ] A phone on mobile data (Wi-Fi off) joins, answers, and its team appears on the big screen
      (this uses the WebSockets).
- [ ] Pictures show on the big screen and phones.
- [ ] `sudo systemctl restart quaesito`: the quiz is still there, the game comes back paused, phones
      say "Reconnecting…" and then carry on.
- [ ] `sudo reboot`: once the server is back, both services run (`systemctl status quaesito caddy`) and
      the site loads.
- [ ] A backup restores (try it on a copy).

## Security notes

- The admin password protects everything but playing: quizzes, the big screen and its controls.
  Uploads also need the server's own machine (see [above](#pictures-soundtracks-and-videos)), so
  someone who learns the password still cannot put files on the server.
- Admin logins last 12 hours, across restarts. To log every browser out sooner, stop the service,
  delete `/var/lib/quaesito/admin-tokens.json` and start it again.
- Anyone with a game's join code can join it as a team, up to the game's team limit. A client that
  asks for 30 codes no game has within five minutes waits five minutes, as one with 10 wrong passwords
  does, so codes cannot practically be found by trying. Share the code only with your players; once
  every team is in, untick **New teams can join** under **Teams** on the big screen, where you can
  also remove unwanted teams.
  End games you no longer need.
- Pictures, soundtracks and videos are served to anyone who has their address, which is random and
  only appears in the pages of the quizzes using them.
- Caddy writes no access log unless you add a `log` directive. If you do, note that admin addresses
  (uploads, exports, the big screen's connection) carry the admin login in a `token` query parameter:
  remove it from the log with

    ```caddy
    log {
    	format filter {
    		request>uri query {
    			delete token
    		}
    	}
    }
    ```

- A team keeps at most three live connections (a new one closes its oldest), and its actions are
  paced to ten a second, so one phone cannot keep the server busy for everyone. A message on a
  connection can still be up to 64 MB. Beyond this, Caddy and quaesito do not limit how many
  requests a client makes. For a public server
  that attracts unwanted traffic, consider a provider's firewall rules or Caddy
  [rate limiting](https://github.com/mholt/caddy-ratelimit) (a plugin).

## Oracle Cloud

Oracle Cloud's Always Free tier includes ARM64 (Ampere A1) virtual machines, enough for quaesito.
Check Oracle's current free limits, and keep the instance within them.

1. **Create the instance:** Compute > Instances > Create instance. Image: Canonical Ubuntu (22.04 or
   later); shape: VM.Standard.A1.Flex, 1 OCPU and 6 GB of memory (more if your free allowance allows).
   Add your SSH public key, and keep "Assign a public IPv4 address" on.
2. **Keep its address:** under the instance's VNIC > IPv4 addresses, change the public IP to a
   *reserved* one, so it survives stopping or recreating the instance. Point your hostname at it.
3. **Open ports 80 and 443 in Oracle's firewall:** Networking > Virtual cloud networks > your VCN >
   Security Lists (or the subnet's network security group) > Add ingress rules: source `0.0.0.0/0`,
   TCP, destination ports `80` and `443`. Do not add 8080.
4. **Open them on the instance:** Oracle's Ubuntu images come with iptables rules that refuse
   everything but SSH. Add the two ports before the final `REJECT` rule, and save:

    ```sh
    ssh ubuntu@<public-ip>
    sudo iptables -L INPUT --line-numbers   # note the number of the REJECT line, e.g. 5
    sudo iptables -I INPUT 5 -p tcp -m state --state NEW -m multiport --dports 80,443 -j ACCEPT
    sudo netfilter-persistent save
    ```

5. Follow [Install](#install), using the `linux-arm64` archive.

## Building from source

For a version that has no release yet, build on the server (ARM64 included) or on any Linux computer of
the same processor type. It needs a few GB of memory and takes a while the first time.

```sh
sudo apt install -y build-essential pkg-config libssl-dev git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.10 --locked
git clone https://github.com/ggirelli/quaesito && cd quaesito
dx bundle --platform web --release --out-dir dist
sudo mkdir -p /opt/quaesito && sudo cp -R dist/. packaging/deploy /opt/quaesito/
```

The user guide is optional (without it, **Help** opens the online one); to include it, build it as
`just bundle` does. Then continue with [Install](#install) from step 2.

To try quaesito behind HTTPS without a public hostname, run Caddy with its own certificate authority
(`tls internal` in the site block): browsers warn about the certificate, so this is for testing only.
