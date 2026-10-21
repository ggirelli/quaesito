#!/usr/bin/env bash
# Tag a new version of quaesito: update its dependencies, then set it.
#
# Run as `just tag [--no-check] major|minor|patch|X.Y.Z`, on a clean working
# copy, it:
#
# 1. bumps the package's version (`just version`), or takes the one given,
#    which must be higher, asking first;
# 2. upgrades Cargo.lock's dependencies within Cargo.toml's ranges, keeping
#    Dioxus at dx's version (the justfile's `dx-version`), committing them,
#    if any changed, as "build: update dependencies";
# 3. runs the checks (`just check`, unless `--no-check`);
# 4. sets the version in Cargo.toml's `[package]` and Cargo.lock,
#    moves the CHANGELOG's Unreleased section under `## [X.Y.Z] - <today>`,
#    and commits them as "build: tag vX.Y.Z".
#
# It commits with jj in a jj repository (signing the commits, if jj signs),
# else with git. Open a pull request with them; once it's merged, tag main
# (see CONTRIBUTING.md, Releasing).
set -euo pipefail

cd "$(dirname "$0")/../.."

usage="usage: just tag [--no-check] major|minor|patch|X.Y.Z"
semver='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'

fail() {
    echo "$1" >&2
    exit 1
}

checks=true
bump=""
for arg in "$@"; do
    case "$arg" in
    --no-check) checks=false ;;
    -h | --help)
        echo "$usage"
        exit 0
        ;;
    -*) fail "$usage" ;;
    *)
        [ -z "$bump" ] || fail "$usage"
        bump="$arg"
        ;;
    esac
done
[ -n "$bump" ] || fail "$usage"

if [ -d .jj ]; then
    vcs=jj
    clean=$([ "$(jj log -r @ --no-graph -T empty)" = true ] && echo yes || true)
else
    vcs=git
    clean=$([ -z "$(git status --porcelain)" ] && echo yes || true)
fi
[ -n "$clean" ] || fail "The working copy has changes: commit them first."

# Commit $2... as $1, signing it if jj signs but didn't: main requires
# signed commits, and signing isn't automatic everywhere.
commit() {
    local message=$1
    shift
    if [ "$vcs" = git ]; then
        git add -- "$@"
        git commit -m "$message" -- "$@"
        return
    fi
    jj commit -m "$message" "$@"
    local backend
    backend=$(jj config get signing.backend 2>/dev/null || true)
    if [ -n "$backend" ] && [ "$backend" != none ] &&
        [ "$(jj log -r @- --no-graph -T 'if(signature, "signed")')" != signed ]; then
        jj sign -r @-
    fi
}

current=$(just version)
[[ "$current" =~ $semver ]] || fail "Cargo.toml's version $current isn't X.Y.Z."
major=${BASH_REMATCH[1]} minor=${BASH_REMATCH[2]} patch=${BASH_REMATCH[3]}
case "$bump" in
major) version="$((major + 1)).0.0" ;;
minor) version="$major.$((minor + 1)).0" ;;
patch) version="$major.$minor.$((patch + 1))" ;;
*)
    [[ "$bump" =~ $semver ]] || fail "$bump is neither major, minor, patch nor X.Y.Z."
    version=$bump
    highest=$(printf '%s\n%s\n' "$current" "$version" | sort -V | tail -n 1)
    [ "$version" != "$current" ] && [ "$highest" = "$version" ] ||
        fail "$version isn't higher than $current."
    ;;
esac
just release-notes Unreleased >/dev/null 2>&1 ||
    fail "The CHANGELOG's Unreleased section is empty: nothing to release."

read -r -p "Tag $version (after $current)? [y/N] " answer
case "$answer" in
y | Y | yes | Yes) ;;
*) fail "Not tagged." ;;
esac

dx_version=$(sed -n 's/^dx-version := "\(.*\)"$/\1/p' justfile)
[ -n "$dx_version" ] || fail "The justfile has no dx-version."
lock_before=$(sha256sum Cargo.lock)
cargo update
# dx builds the UI at its own version: a newer Dioxus would break `just serve` and `just bundle`.
cargo update -p dioxus --precise "$dx_version"
if [ "$(sha256sum Cargo.lock)" != "$lock_before" ]; then
    commit "build: update dependencies" Cargo.lock
fi
if [ "$checks" = true ]; then
    just check
fi

# The first `version = ` under `[package]`, which `just version` reads.
awk -v v="$version" '
    /^\[/ { section = $0 }
    section == "[package]" && !done && /^version = / {
        print "version = \"" v "\""; done = 1; next
    }
    { print }
' Cargo.toml >Cargo.toml.new
mv Cargo.toml.new Cargo.toml
[ "$(just version)" = "$version" ] || fail "Couldn't set the version in Cargo.toml."
cargo update --workspace
awk -v v="$version" -v d="$(date +%F)" '
    { print }
    $0 == "## [Unreleased]" { print ""; print "## [" v "] - " d }
' CHANGELOG.md >CHANGELOG.md.new
mv CHANGELOG.md.new CHANGELOG.md
just release-notes "$version" >/dev/null
commit "build: tag v$version" Cargo.toml Cargo.lock CHANGELOG.md

echo "Tagged $version: open a pull request with its commits, then tag main once it's merged."
