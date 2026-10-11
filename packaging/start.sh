#!/bin/sh
# Starts quaesito for a quiz night. Phones on the same Wi-Fi can join.
# Leave this window open while you play; press Ctrl+C to stop.
cd "$(dirname "$0")" || exit 1
export IP="${IP:-0.0.0.0}"
export PORT="${PORT:-8080}"
export QUAESITO_DATA_DIR="${QUAESITO_DATA_DIR:-$PWD/data}"
url="http://localhost:$PORT"
echo "Starting quaesito on $url"
echo "Your quizzes and pictures are kept in $QUAESITO_DATA_DIR"
# Open the browser once the server is up.
( sleep 2; xdg-open "$url" >/dev/null 2>&1 || open "$url" >/dev/null 2>&1 ) &
exec ./server
