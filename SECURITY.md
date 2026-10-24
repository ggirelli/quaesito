# Security policy

## Supported versions

Quaesito is developed on the `main` branch; security fixes go into the latest release and `main`.

## Reporting a vulnerability

Please **do not open a public issue**. Report it privately:

- through GitHub's private vulnerability reporting: the repository's **Security** tab →
  **Report a vulnerability** (available while the repository is public), or
- by contacting the maintainer, [@ggirelli](https://github.com/ggirelli), on GitHub and asking for
  a private channel, without details in public.

Include the steps to reproduce it and the impact you expect. You will get an answer within a few
days, and credit in the release notes if you would like it.

## What to expect from quaesito

Quaesito is meant to run on a trusted local network for a game night:

- The admin area is protected by a single password (`QUAESITO_ADMIN_PASSWORD`, or a generated
  one printed at startup). Admin tokens are kept in the data directory, readable by its owner only.
- Team leaders' phones get a per-team ticket; anyone on the network who knows a game's code can
  join it.
- Uploaded pictures are decoded and re-encoded before they are stored; soundtracks are only
  accepted in recognised audio formats, and imported quiz bundles go through the same checks.
- Traffic is plain HTTP. Do not expose the server to the internet without a reverse proxy that
  adds TLS, and choose a strong admin password if you do. TLS protects the traffic, not quaesito
  itself: the code has not been audited, and exposing it remains at your own risk. See
  [hosting on the internet](https://ggirelli.github.io/quaesito/deployment/) for the setup, its
  limits and its security notes.
