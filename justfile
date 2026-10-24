# docs/requirements.txt pins MkDocs below 2.0, so Material's banner warning about MkDocs 2.0 is noise
export NO_MKDOCS_2_WARNING := "true"

# The dioxus-cli version that builds the app; Cargo.lock keeps Dioxus at the same version (see `just tag`)
dx-version := "0.7.10"

# List available recipes
default:
    @just --list

# Run the app with hot reload, reachable from phones on the LAN (with a fresh user guide at /docs/)
serve port="8080": docs-build
    QUAESITO_ADMIN_PASSWORD="${QUAESITO_ADMIN_PASSWORD:-quaesito}" dx serve --platform web --addr 0.0.0.0 --port {{port}}

# Build an optimized release bundle into ./dist, with the user guide in ./dist/docs as in the downloads
bundle:
    dx bundle --platform web --release --out-dir dist
    rm -rf dist/docs
    uvx --with-requirements docs/requirements.txt mkdocs build --strict --site-dir dist/docs

# Build and run the release bundle on the LAN; quizzes and pictures live in ./data
play port="8080": bundle
    cd dist && IP=0.0.0.0 PORT={{port}} QUAESITO_DATA_DIR="{{justfile_directory()}}/data" ./server

# Format all code
fmt:
    cargo fmt --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Lint the server build (every crate of the workspace)
clippy-server:
    cargo clippy --workspace --no-default-features --features quaesito/server --all-targets --locked -- -D warnings

# Lint the wasm client build (the app and the crates it builds into the browser)
clippy-web:
    cargo clippy --target wasm32-unknown-unknown -p quaesito -p quaesito-core --all-targets --locked -- -D warnings

# Lint both builds
clippy: clippy-server clippy-web

# Check the API docs of both builds, failing on warnings (such as links to private items)
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --no-default-features --features quaesito/server --locked
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --target wasm32-unknown-unknown -p quaesito -p quaesito-core --locked

# Test the rules crate on its own, and check it builds for the browser (no server code)
core:
    cargo test -p quaesito-core --locked
    cargo check -p quaesito-core --target wasm32-unknown-unknown --locked

# Run the test suite of every crate
test:
    cargo test --workspace --no-default-features --features quaesito/server --locked

# Rewrite the published schema of the quiz file format (docs/schema) from the quiz types
schema:
    QUAESITO_WRITE_SCHEMA=1 cargo test -p quaesito-server --locked the_published_schema_is_current

# Copy the question edits saved in the app (data/library-edits.json) into the library's topic files
library-apply-edits edits="data/library-edits.json":
    uv run tools/library/apply-edits.py {{edits}}

# Check advisories, licenses, bans and sources
deny:
    cargo deny --locked check

# Check that no source file is over 1,000 lines (see CONTRIBUTING.md, "Modules")
file-length:
    tools/check/file-length.sh

# Everything CI checks
check: fmt-check file-length clippy doc test core deny

# Print the workspace's version
version:
    @sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1

# Print a version's section of the CHANGELOG (X.Y.Z or Unreleased), failing if it's missing or empty
release-notes version:
    #!/usr/bin/env bash
    set -euo pipefail
    notes=$(awk -v v="{{version}}" '
      /^## \[/ { if (on) exit; on = index($0, "[" v "]") > 0; next }
      on { print }
    ' CHANGELOG.md)
    [ -n "$(tr -d '[:space:]' <<<"$notes")" ] || { echo "The CHANGELOG has no notes for {{version}}." >&2; exit 1; }
    printf '%s\n' "$notes"

# Bump the version (major, minor, patch or X.Y.Z) once the dependencies are updated and checked, committing both (see tools/release/tag.sh)
tag *args:
    tools/release/tag.sh {{args}}

# Serve the documentation site with live reload at http://127.0.0.1:8000
docs:
    uvx --with-requirements docs/requirements.txt mkdocs serve --livereload

# Build the documentation site into ./site, failing on warnings (as CI does)
docs-build:
    uvx --with-requirements docs/requirements.txt mkdocs build --strict

# Retake the documentation screenshots (plays a sample quiz in a headless browser)
screenshots:
    cd tools/browser && npm install --no-audit --no-fund >/dev/null
    rm -rf target/e2e/data
    tools/browser/run.sh screenshots.js

# Record the demo videos (playing a quiz, building one, the question library) with their
# voice-overs, captions and transcripts (needs ffmpeg). The voice is chatterbox (expressive, best
# with an NVIDIA GPU) or kokoro (quick on any computer).
demo-video voice="chatterbox":
    #!/usr/bin/env bash
    set -euo pipefail
    (cd tools/browser && npm install --no-audit --no-fund >/dev/null)
    case "{{voice}}" in
      chatterbox)
        # perth, its watermarker, still imports pkg_resources, gone from setuptools 81.
        uvx --python 3.11 --with chatterbox-tts --with faster-whisper --with soundfile --with 'setuptools<81' \
          python tools/browser/voice.py chatterbox ;;
      kokoro)
        mkdir -p target/kokoro
        for f in kokoro-v1.0.onnx voices-v1.0.bin; do
          test -f "target/kokoro/$f" || curl -sSL -o "target/kokoro/$f" \
            "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/$f"
        done
        uvx --python 3.12 --with kokoro-onnx --with soundfile python tools/browser/voice.py kokoro ;;
      *) echo "voice must be chatterbox or kokoro" >&2; exit 1 ;;
    esac
    for script in demo-video.js editor-video.js library-video.js; do
      rm -rf target/e2e/data
      tools/browser/run.sh "$script"
    done
