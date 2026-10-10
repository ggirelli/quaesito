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
