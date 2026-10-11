#!/bin/sh
# Starts quaesito for a quiz night (double-click in Finder). Phones on the same Wi-Fi can join.
# Leave this window open while you play; close it to stop.
cd "$(dirname "$0")" || exit 1
# Downloads are quarantined by macOS; allow the bundled server to run.
xattr -dr com.apple.quarantine . 2>/dev/null
export IP="${IP:-0.0.0.0}"
export PORT="${PORT:-8080}"
export QUAESITO_DATA_DIR="${QUAESITO_DATA_DIR:-$PWD/data}"
url="http://localhost:$PORT"
echo "Starting quaesito on $url"
echo "Your quizzes and pictures are kept in $QUAESITO_DATA_DIR"
( sleep 2; open "$url" ) &
exec ./server
