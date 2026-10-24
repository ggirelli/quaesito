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
- Python 3 (`python3`), only for `just playground-serve` and `just playground-check`, which serve
  the [Playground](#the-playground) with its built-in web server

## Common commands

```sh
git clone https://github.com/ggirelli/quaesito
cd quaesito
just serve   # hot-reloading dev server on the LAN; admin password "quaesito"
just test    # the test suite
just core    # the rules crate alone, and its wasm build
just check   # formatting, lints, tests and licence checks, as in CI
just play    # release build served on the LAN, with its data in ./data
just docs    # this site, live at http://127.0.0.1:8000
```

`just` on its own lists every command.

## How the code is organised

The repository is a Cargo workspace of four crates, each depending only on the ones below it:

- **`quaesito-playground`** (`crates/playground`): the [Playground](https://ggirelli.github.io/quaesito/playground/), one quiz played in the
  browser alone (see [below](#the-playground)).
- **`quaesito`** (the root: `src/`): the app. `main.rs` starts it, `src/ui/` holds the pages and
  screens, and `src/api.rs` declares the server functions the browser calls. Built for the browser
  (the default `web` feature) and for the server (`--features server`, which adds `quaesito-server`).
  The pages that talk to a server need its `fullstack` feature (both builds turn it on); without it,
  only the big screen's and the phones' parts and the Playground are built.
- **`quaesito-server`** (`crates/server`): everything that needs a server: quiz, media and
  library storage, admin sign-in, live sessions and their WebSockets, HTTP routes. Its `build.rs`
  bundles the question library from `library/`.
- **`quaesito-core`** (`crates/core`): the rules, with no server, runtime, files or network: quizzes
  and their validation, the messages of a live game, scoring, typed-answer matching and the game
  engine. Time and randomness are passed in, so its tests need no tricks; it also builds for the
  browser. `just core` tests it on its own and checks that it builds for wasm.

## The Playground

The [Playground](https://ggirelli.github.io/quaesito/playground/) plays one quiz (`crates/playground/src/demo-quiz.json`) with no server: the
game runs in the browser, with `quaesito-core`'s engine, and drives the app's own big screen and
phone, side by side as on the test page (`src/ui/playground/`). It is built without `fullstack`,
which would make the browser wait for a server-rendered page, and published with this guide at
`/quaesito/playground/` by the docs workflow.

```sh
just playground         # build it as published, into target/pages/quaesito/playground
just playground-serve   # serve that at http://127.0.0.1:8094/quaesito/playground/
just playground-check   # play its quiz to the end in a headless browser, as CI does
```

`just clippy` lints its build on its own (`just clippy-playground`): built with the other crates,
Cargo would turn `fullstack` back on.

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
raise `QUIZ_FORMAT` in `crates/core/src/model/quiz.rs` first, so the new schema is published next to the
old one.

## Contributing

- **Bugs and ideas:** open an [issue](https://github.com/ggirelli/quaesito/issues/new/choose)
  with its template (bug report, idea or feature request, wrong library question).
- **Security problems:** report them privately, as the
  [security policy](https://github.com/ggirelli/quaesito/blob/main/SECURITY.md) explains.
- **Changes:** see [CONTRIBUTING.md](https://github.com/ggirelli/quaesito/blob/main/CONTRIBUTING.md)
  for the workflow, checks and conventions.
- **How it works inside:** see [How it works](architecture.md).
