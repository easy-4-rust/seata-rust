#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

if is_running; then
  echo "refusing to reset storage while Java TC is running (pid=$(cat "$TC_PID_FILE"))" >&2
  exit 2
fi

rm -rf "$TC_DATA_DIR"
mkdir -p "$TC_DATA_DIR"
echo "Reset isolated FILE storage: $TC_DATA_DIR"
