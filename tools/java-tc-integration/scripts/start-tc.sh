#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

if is_running; then
  echo "Java TC already running: pid=$(cat "$TC_PID_FILE") port=$TC_PORT"
  exit 0
fi

bash "$SCRIPT_DIR/build-tc.sh"
mkdir -p "$TC_DATA_DIR" "$TC_LOG_DIR"

export SEATA_TC_WORK_DIR="$TC_WORK_DIR"
export SPRING_CONFIG_LOCATION="file:$TC_CONFIG"
nohup java -jar "$TC_JAR" >"$TC_LOG_DIR/console.log" 2>&1 &

pid=$!
echo "$pid" > "$TC_PID_FILE"

if ! wait_for_tcp "$TC_PORT" 90; then
  echo "Java TC startup failed; see $TC_LOG_DIR/console.log" >&2
  tail -n 120 "$TC_LOG_DIR/console.log" >&2 || true
  exit 1
fi

echo "Java TC is listening on 127.0.0.1:$TC_PORT (pid=$pid)"
echo "Data: $TC_DATA_DIR"
echo "Logs: $TC_LOG_DIR"
