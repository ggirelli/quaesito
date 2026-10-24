# docs/requirements.txt pins MkDocs below 2.0, so Material's banner warning about MkDocs 2.0 is noise
export NO_MKDOCS_2_WARNING := "true"

# The dioxus-cli version that builds the app; Cargo.lock keeps Dioxus at the same version (see `just tag`)
dx-version := "0.7.10"

# List recipes
default:
    @just --list --unsorted

# Run the app with hot reload on the LAN
[group('run')]
serve port="8080": docs-build
    QUAESITO_ADMIN_PASSWORD="${QUAESITO_ADMIN_PASSWORD:-quaesito}" dx serve --platform web --addr 0.0.0.0 --port {{port}}

# Build the release bundle into ./dist
[group('run')]
bundle:
    dx bundle --platform web --release --out-dir dist
    rm -rf dist/docs
    uvx --with-requirements docs/requirements.txt mkdocs build --strict --site-dir dist/docs

# Build and run the release bundle on the LAN
[group('run')]
play port="8080": bundle
    cd dist && IP=0.0.0.0 PORT={{port}} QUAESITO_DATA_DIR="{{justfile_directory()}}/data" ./server

# Build the Playground into ./target/pages
[group('playground')]
playground:
    #!/usr/bin/env bash
    set -euo pipefail
    target="${CARGO_TARGET_DIR:-target}"
    # dx keeps the files of earlier builds in its output: start afresh.
    rm -rf "$target/dx/quaesito-playground/release/web/public" "$target/playground" target/pages
    dx bundle --platform web --release -p quaesito-playground --out-dir "$target/playground"
    mkdir -p target/pages/quaesito
    cp -r "$target/playground/public" target/pages/quaesito/playground

# Serve the Playground at /quaesito/playground/
[group('playground')]
playground-serve port="8094": playground
    cd target/pages && python3 -m http.server {{port}} --bind 127.0.0.1

# Play the Playground in a headless browser
[group('playground')]
playground-check port="8094": playground
    #!/usr/bin/env bash
    set -euo pipefail
    (cd tools/browser && npm install --no-audit --no-fund >/dev/null)
    (cd target/pages && exec python3 -m http.server {{port}} --bind 127.0.0.1 >/dev/null 2>&1) &
    server=$!
    trap 'kill "$server" 2>/dev/null' EXIT
    sleep 1
    PLAYGROUND="http://127.0.0.1:{{port}}/quaesito/playground/" node tools/browser/check-playground.js

# Format the code
[group('checks')]
fmt:
    cargo fmt --all

# Check formatting
[group('checks')]
fmt-check:
    cargo fmt --all -- --check

# Lint the server build
[group('checks')]
clippy-server:
    cargo clippy --workspace --no-default-features --features quaesito/server --all-targets --locked -- -D warnings

# Lint the browser build
[group('checks')]
clippy-web:
    cargo clippy --target wasm32-unknown-unknown -p quaesito -p quaesito-core --all-targets --locked -- -D warnings

# Lint the Playground build (no fullstack)
[group('checks')]
clippy-playground:
    cargo clippy --target wasm32-unknown-unknown -p quaesito-playground --all-targets --locked -- -D warnings

# Lint every build
[group('checks')]
clippy: clippy-server clippy-web clippy-playground

# Check the API docs
[group('checks')]
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --no-default-features --features quaesito/server --locked
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --target wasm32-unknown-unknown -p quaesito -p quaesito-core --locked

# Test the core crate and its wasm build
[group('checks')]
core:
    cargo test -p quaesito-core --locked
    cargo check -p quaesito-core --target wasm32-unknown-unknown --locked

# Run every test
[group('checks')]
test:
    cargo test --workspace --no-default-features --features quaesito/server --locked

# Regenerate the quiz file schema
[group('quiz data')]
schema:
    QUAESITO_WRITE_SCHEMA=1 cargo test -p quaesito-server --locked the_published_schema_is_current

# Apply saved question edits to the library
[group('quiz data')]
library-apply-edits edits="data/library-edits.json":
    uv run tools/library/apply-edits.py {{edits}}

# Check advisories, licences and sources
[group('checks')]
deny:
    cargo deny --locked check

# Check no source file exceeds 1,000 lines
[group('checks')]
file-length:
    tools/check/file-length.sh

# Run everything CI checks
[group('checks')]
check: fmt-check file-length clippy doc test core deny

# Print the version
[group('release')]
version:
    @sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1

# Print a version's CHANGELOG notes
[group('release')]
release-notes version:
    #!/usr/bin/env bash
    set -euo pipefail
    notes=$(awk -v v="{{version}}" '
      /^## \[/ { if (on) exit; on = index($0, "[" v "]") > 0; next }
      on { print }
    ' CHANGELOG.md)
    [ -n "$(tr -d '[:space:]' <<<"$notes")" ] || { echo "The CHANGELOG has no notes for {{version}}." >&2; exit 1; }
    printf '%s\n' "$notes"

# Bump the version and commit
[group('release')]
tag *args:
    tools/release/tag.sh {{args}}

# Serve the docs with live reload
[group('docs')]
docs:
    uvx --with-requirements docs/requirements.txt mkdocs serve --livereload

# Build the docs into ./site
[group('docs')]
docs-build:
    uvx --with-requirements docs/requirements.txt mkdocs build --strict

# Retake the docs screenshots
[group('docs')]
screenshots:
    cd tools/browser && npm install --no-audit --no-fund >/dev/null
    rm -rf target/e2e/data
    tools/browser/run.sh screenshots.js

# Record the demo videos (needs ffmpeg)
[group('docs')]
demo-video voice="chatterbox":
    #!/usr/bin/env bash
    set -euo pipefail
    (cd tools/browser && npm install --no-audit --no-fund >/dev/null)
    # chatterbox is expressive (best with an NVIDIA GPU); kokoro is quick on any computer.
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
