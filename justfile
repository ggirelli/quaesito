# List available recipes
default:
    @just --list

# Run the app locally with hot reload
serve:
    dx serve --platform web

# Build an optimized release bundle
bundle:
    dx bundle --platform web --release

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
