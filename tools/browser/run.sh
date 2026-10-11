#!/bin/sh
# Builds quaesito into target/e2e, serves it on $PORT (default 8093) with its own data directory,
# runs one script from this folder against it, then stops that server.
#
#   tools/browser/run.sh check-pins.js [args…]
#
# The build and data stay apart from `just serve`, so both can run at once.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
port=${PORT:-8093}
target="$root/target/e2e"
cd "$root"
CARGO_TARGET_DIR="$target" dx build --platform web >"$target.build.log" 2>&1 || { tail -20 "$target.build.log"; exit 1; }
mkdir -p "$target/data"
# Scripts set up their own quizzes: skip the demo quiz a fresh server adds, unless DEMO_QUIZ=1.
[ -n "${DEMO_QUIZ:-}" ] || : >"$target/data/demo-quiz-added"
QUAESITO_DATA_DIR="$target/data" QUAESITO_ADMIN_PASSWORD=test IP=127.0.0.1 PORT="$port" \
  "$target/dx/quaesito/debug/web/server" >"$target.server.log" 2>&1 &
server=$!
trap 'kill "$server" 2>/dev/null' EXIT
sleep 2
script=$1
shift
cd "$here"
BASE="http://127.0.0.1:$port" node "$script" "$@"
