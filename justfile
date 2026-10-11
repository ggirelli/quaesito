# List available recipes
default:
    @just --list

# Run the app with hot reload, reachable from phones on the LAN
serve port="8080":
    QUAESITO_ADMIN_PASSWORD="${QUAESITO_ADMIN_PASSWORD:-quaesito}" dx serve --platform web --addr 0.0.0.0 --port {{port}}

# Build an optimized release bundle into ./dist
bundle:
    dx bundle --platform web --release --out-dir dist

# Build and run the release bundle on the LAN; quizzes and pictures live in ./data
play port="8080": bundle
    cd dist && IP=0.0.0.0 PORT={{port}} QUAESITO_DATA_DIR="{{justfile_directory()}}/data" ./server

# Format all code
fmt:
    cargo fmt --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Lint the server build
clippy-server:
    cargo clippy --no-default-features --features server --all-targets --locked -- -D warnings

# Lint the wasm client build
clippy-web:
    cargo clippy --target wasm32-unknown-unknown --all-targets --locked -- -D warnings

# Lint both builds
clippy: clippy-server clippy-web

# Run the test suite
test:
    cargo test --no-default-features --features server --locked

# Check advisories, licenses, bans and sources
deny:
    cargo deny --locked check

# Everything CI checks
check: fmt-check clippy test deny

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

# Record the demo video with its Kokoro voice-over, captions and transcript (needs ffmpeg)
demo-video:
    cd tools/browser && npm install --no-audit --no-fund >/dev/null
    mkdir -p target/kokoro
    test -f target/kokoro/kokoro-v1.0.onnx || curl -sSL -o target/kokoro/kokoro-v1.0.onnx https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/kokoro-v1.0.onnx
    test -f target/kokoro/voices-v1.0.bin || curl -sSL -o target/kokoro/voices-v1.0.bin https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin
    uvx --python 3.12 --with kokoro-onnx --with soundfile python tools/browser/voice.py
    rm -rf target/e2e/data
    tools/browser/run.sh demo-video.js
