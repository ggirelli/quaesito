# Development

Quaesito is written in Rust with [Dioxus](https://dioxuslabs.com): one program serves the web
app to the big screen and the phones, and keeps the games' state.

## What you need

- Rust (stable), with the `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [`dx`](https://dioxuslabs.com/learn/0.7/getting_started), the Dioxus CLI, at the version in the
  justfile's `dx-version` (0.7.10): `cargo install dioxus-cli --version 0.7.10 --locked`
- [`just`](https://github.com/casey/just), which runs the commands below
- [`uv`](https://docs.astral.sh/uv/), which builds this guide: `just serve`, `just play` and
  `just bundle` build it too
- [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny), for `just check`
- Node.js and npm, only for the browser checks, screenshots and demo videos in `tools/browser`

## Common commands

```sh
git clone https://github.com/ggirelli/quaesito
cd quaesito
just serve   # hot-reloading dev server on the LAN; admin password "quaesito"
just test    # the test suite
just check   # formatting, lints, tests and licence checks, as in CI
just play    # release build served on the LAN, with its data in ./data
just docs    # this site, live at http://127.0.0.1:8000
```

`just` on its own lists every command.

## Running it from source

For systems the [downloads](getting-started.md#which-download) do not cover. With the tools
above, `just play` builds a release and serves it on the LAN, as the start scripts do: quizzes and
pictures are kept in `data` in the checkout. CI does not build or test these systems. On a Linux server, see
[building from source](deployment.md#building-from-source).

## The user guide

The app serves this guide at `/docs/`: `just serve` and `just play` build it first, from the same
checkout as the app, so it always describes the code you run (the server warns if `site/` falls
behind its sources, say after editing the docs while `dx serve` runs: `just docs-build` refreshes
it). The downloads ship the guide built from their release. This site follows `main`, so it can
describe features that are not released yet. `just docs-build` fails on any warning, as CI does.

The screenshots on this site come from `just screenshots` (`tools/browser/run.sh screenshots.js`),
which plays a sample quiz in a headless browser; the demo videos come from `just demo-video`.

## The quiz file format

The [quiz file format](quiz-file-format.md) schema in `docs/schema` is generated from the quiz types,
and a test fails when they drift apart. After changing what a quiz holds, `just schema` rewrites it;
if an older version would misread or lose the change (a new kind of question, a new setting value),
raise `QUIZ_FORMAT` in `src/model.rs` first, so the new schema is published next to the old one.

## Contributing

- **Bugs and ideas:** open an [issue](https://github.com/ggirelli/quaesito/issues/new/choose)
  with its template (bug report, idea or feature request, wrong library question).
- **Security problems:** report them privately, as the
  [security policy](https://github.com/ggirelli/quaesito/blob/main/SECURITY.md) explains.
- **Changes:** see [CONTRIBUTING.md](https://github.com/ggirelli/quaesito/blob/main/CONTRIBUTING.md)
  for the workflow, checks and conventions.
- **How it works inside:** [DESIGN.md](https://github.com/ggirelli/quaesito/blob/main/planning/DESIGN.md);
  progress is tracked in [STATUS.md](https://github.com/ggirelli/quaesito/blob/main/planning/STATUS.md).
