#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
JAVA_SHA="cba73e180fd8896a99df6415b859d275122feaa7"
TC_WORK_DIR="${SEATA_TC_WORK_DIR:-${TMPDIR:-/tmp}/seata-rust-java-tc}"
mkdir -p "$TC_WORK_DIR"
TC_WORK_DIR="$(cd "$TC_WORK_DIR" && pwd -P)"
case "$TC_WORK_DIR" in
  /|"$HOME"|"$ROOT")
    echo "unsafe SEATA_TC_WORK_DIR: $TC_WORK_DIR" >&2
    exit 2
    ;;
esac
TC_SOURCE_DIR="$TC_WORK_DIR/seata-java"
TC_DATA_DIR="$TC_WORK_DIR/data"
TC_LOG_DIR="$TC_WORK_DIR/logs"
TC_PID_FILE="$TC_WORK_DIR/seata-tc.pid"
TC_PORT="${SEATA_TC_PORT:-18091}"
TC_CONFIG="$ROOT/tools/java-tc-integration/config/application.yml"
TC_JAR="$TC_SOURCE_DIR/server/target/seata-server-exec.jar"

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "required command not found: $1" >&2
    exit 2
  }
}

require_command git
require_command java

if [[ ! -f "$TC_CONFIG" ]]; then
  echo "missing TC config: $TC_CONFIG" >&2
  exit 2
fi

is_running() {
  [[ -f "$TC_PID_FILE" ]] || return 1
  local pid
  pid="$(cat "$TC_PID_FILE")"
  [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null
}

wait_for_tcp() {
  local port="$1"
  local attempts="${2:-90}"
  for ((i=1; i<=attempts; i++)); do
    if (echo >"/dev/tcp/127.0.0.1/$port") >/dev/null 2>&1; then
      return 0
    fi
    if [[ -f "$TC_PID_FILE" ]] && ! is_running; then
      echo "Java TC process exited before opening TCP port $port" >&2
      [[ -f "$TC_LOG_DIR/seata-server.log" ]] && tail -n 120 "$TC_LOG_DIR/seata-server.log" >&2
      return 1
    fi
    sleep 2
  done
  echo "timed out waiting for Java TC TCP port $port" >&2
  [[ -f "$TC_LOG_DIR/seata-server.log" ]] && tail -n 120 "$TC_LOG_DIR/seata-server.log" >&2
  return 1
}
