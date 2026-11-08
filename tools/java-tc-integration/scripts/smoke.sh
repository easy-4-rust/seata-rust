#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

cleanup() {
  bash "$SCRIPT_DIR/stop-tc.sh" || true
}
trap cleanup EXIT

bash "$SCRIPT_DIR/reset-storage.sh"
bash "$SCRIPT_DIR/start-tc.sh"

# Verify the listener first, then use the standalone Rust client to exchange
# native Seata V1 RegisterTM and RegisterRM messages with the live TC.
wait_for_tcp "$TC_PORT" 5
echo "PASS: pinned Java TC process is alive and TCP port $TC_PORT accepts connections"
bash "$SCRIPT_DIR/run-rust-registration.sh"
