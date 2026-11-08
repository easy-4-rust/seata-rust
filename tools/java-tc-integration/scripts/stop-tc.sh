#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

if ! is_running; then
  rm -f "$TC_PID_FILE"
  echo "Java TC is not running"
  exit 0
fi

pid="$(cat "$TC_PID_FILE")"
kill -TERM "$pid"
for ((i=1; i<=30; i++)); do
  if ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$TC_PID_FILE"
    echo "Java TC stopped"
    exit 0
  fi
  sleep 1
done

echo "Java TC did not stop within 30 seconds; sending SIGKILL" >&2
kill -KILL "$pid" 2>/dev/null || true
rm -f "$TC_PID_FILE"
