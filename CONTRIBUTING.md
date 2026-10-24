# Contributing to quaesito

Thanks for helping! Quaesito is a small, self-hosted quiz app; contributions of every size are
welcome: bug reports, question-type ideas, fixes, docs and translations of the interface.

## Before you start

- **Bugs and ideas:** open an issue with the templates. For a bigger change, describe it in an
  issue first so we can agree on the approach before you spend time on it.
- **Security problems:** do not open a public issue; see [SECURITY.md](SECURITY.md).
- Be kind: this project follows the [code of conduct](CODE_OF_CONDUCT.md).

## Setting up

You need Rust (stable) with the `wasm32-unknown-unknown` target,
[`dx`](https://dioxuslabs.com/learn/0.7/getting_started) (Dioxus CLI 0.7),
[`just`](https://github.com/casey/just), [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny)
and [`uv`](https://docs.astral.sh/uv/) (`just serve` builds the user guide with it). The
[development guide](https://ggirelli.github.io/quaesito/development/) lists them with install
commands.

```sh
rustup target add wasm32-unknown-unknown
just serve   # hot-reloading dev server on the LAN; admin password "quaesito"
```

Open `http://localhost:8080`, click the ⚙ icon, create a quiz, and press **🧪 Test** to play it
with the big screen and two phones side by side.

## Making a change

1. Keep changes focused: one feature or fix per pull request, split into small commits that each
   build and pass the checks.
2. Run the checks CI runs:

   ```sh
   just check   # rustfmt, clippy (server and wasm, pedantic, warnings are errors), rustdoc, tests, cargo-deny
   ```

3. Add or update tests. The game logic lives in `crates/core/src/engine/` and is fully unit tested
   (it takes the time and randomness as inputs, so no tricks are needed); scoring is in
   `crates/core/src/scoring.rs`. `quaesito-core` must stay free of server code: `just core` tests it
   alone and builds it for wasm (see [how the code is organised](docs/development.md#how-the-code-is-organised)).
4. For UI changes, check them in a browser. `tools/browser` has puppeteer helpers that build the
   app into `target/e2e` and serve it on a separate port:

   ```sh
   (cd tools/browser && npm install)
   tools/browser/run.sh check-pins.js assets/brand/lobby.webp
   ```

   CI does not run these checks, so run the ones about the screens you changed.

5. Update the docs that describe what you changed: the user guide in [docs/](docs/) for players
   and hosts (`just docs` previews it), [README.md](README.md) for the overview,
   [how it works](docs/architecture.md) for the design, and an entry in
   [CHANGELOG.md](CHANGELOG.md) under "Unreleased".
6. Open a pull request against `main`. `main` is protected: changes land through pull requests
   (squash or rebase, so history stays linear) once the CI and docs checks pass.

## Releasing

1. On a clean, up-to-date `main`, run `just tag patch` (or `minor`, `major`, or an exact `X.Y.Z`). It
   updates the dependencies within Cargo.toml's ranges (keeping Dioxus at `dx-version`, in the
   justfile), runs `just check`, sets the new version and moves the CHANGELOG's "Unreleased" notes
   under it, in two commits: "build: update dependencies" and "build: tag vX.Y.Z".
2. Open a pull request with them.
3. Once it's merged, tag `main` and push the tag (`git tag vX.Y.Z && git push origin vX.Y.Z`): the
   release workflow builds the downloads and publishes them with the version's CHANGELOG notes.

## Style

### Code

- Code follows `rustfmt` and `clippy::pedantic`; prefer clear names over comments, and comment the
  *why* when it is not obvious.
- Write code a reader can follow from top to bottom: small functions named after what they do, the
  main path first and the details after it, no clever shortcuts where a plain loop or `match` reads
  better.
- Match the code around you: its naming, its idioms and how much it comments.
- Every public item has a doc comment, and every module opens with a `//!` comment saying what it
  is for. Public functions that can fail say when in an `# Errors` section.

### Modules

- A source file stays under 1,000 lines, tests included; `just check` fails on longer ones.
- Split code by subject, not by kind: a module holds one topic (the teams of a game, the reveal of
  an answer, the pairs editor) with its types, functions and components together.
- When a module grows too big, turn it into a folder: `engine.rs` becomes `engine/mod.rs` with
  sibling modules for its topics. Re-export from `mod.rs` what used to be public, so callers keep
  their paths. A type's methods can be spread over the modules of its topics, each in its own
  `impl` block.
- Code that moves keeps its shape: a pull request that moves code does not also change it, so the
  diff (`git diff --color-moved`) shows that nothing else changed.

### Tests

- Unit tests live next to the code they test. Small modules keep them in a `#[cfg(test)] mod tests`
  at the end; big ones put them in a sibling file (`mod tests;` → `tests.rs`, or a `tests/` folder
  split by subject, like the module itself).
- Test names are sentences describing the behaviour (`a_joker_doubles_one_question_once`).
- `quaesito-core` takes the time and randomness as inputs: tests pass them in instead of waiting
  or mocking.

### Words and commits

- Messages shown to people are short, friendly sentences in British English.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat:`, `fix:`, `docs:`…), with a body explaining what changed and why. Pull request titles do
  too, since they become the commit message when the pull request is squash-merged.

## Assets

Pictures and sounds the project needs but cannot make itself are listed in [assets/README.md](assets/README.md),
with prompts for generating them. Only contribute assets you have the right to share under the
project's licence.

By contributing, you agree that your contributions are licensed under the [MIT licence](LICENSE).
