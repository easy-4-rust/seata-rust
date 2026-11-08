#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

cleanup() {
  "$SCRIPT_DIR/stop-tc.sh" || true
}
trap cleanup EXIT

"$SCRIPT_DIR/reset-storage.sh"
"$SCRIPT_DIR/start-tc.sh"

# This is intentionally only a process/TCP-listener smoke test. It does not
# send a Seata V1 registration or transaction message.
wait_for_tcp "$TC_PORT" 5
echo "PASS: pinned Java TC process is alive and TCP port $TC_PORT accepts connections"
