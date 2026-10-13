# Development

Quaesito is written in Rust with [Dioxus](https://dioxuslabs.com): one program serves the web
app to the big screen and the phones, and keeps the games' state.

You need Rust (stable) with the `wasm32-unknown-unknown` target,
[`dx`](https://dioxuslabs.com/learn/0.7/getting_started) (Dioxus CLI 0.7),
[`just`](https://github.com/casey/just), [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny)
and, for these docs, [`uv`](https://docs.astral.sh/uv/).

```sh
git clone https://github.com/ggirelli/quaesito
cd quaesito
just serve   # hot-reloading dev server on the LAN; admin password "quaesito"
just check   # formatting, lints, tests and licence checks, as in CI
just play    # release build served on the LAN
just docs    # this site, live at http://127.0.0.1:8000
```

The app serves this guide at `/docs/`: `just serve` and `just play` build it first, from the same
checkout as the app, so it always describes the code you run (the server warns if `site/` falls
behind its sources, say after editing the docs while `dx serve` runs: `just docs-build` refreshes
it). The downloads ship the guide built from their release. This site follows `main`, so it can
describe features that are not released yet.

The [quiz file format](quiz-file-format.md) schema in `docs/schema` is generated from the quiz types,
and a test fails when they drift apart. After changing what a quiz holds, `just schema` rewrites it;
if an older version would misread or lose the change (a new kind of question, a new setting value),
raise `QUIZ_FORMAT` in `src/model.rs` first, so the new schema is published next to the old one.

See [CONTRIBUTING.md](https://github.com/ggirelli/quaesito/blob/main/CONTRIBUTING.md) for how to
contribute, and
[DESIGN.md](https://github.com/ggirelli/quaesito/blob/main/planning/DESIGN.md) for how it works inside.

The screenshots on this site come from `tools/browser/run.sh screenshots.js`, which plays a sample
quiz in a headless browser.
