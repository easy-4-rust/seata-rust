#!/usr/bin/env bash
set -euo pipefail

# Run fresh Java135 capture, the pinned Go diagnostic, and Rust against the same78 payloads.
# Existing git/Maven/JDK/Python3/Go/Rust toolchains are required; this script never installs them.
ROOT="$(git rev-parse --show-toplevel)"
HANDLER_RUN_DIR="$(mktemp -d)"
trap 'rm -rf "$HANDLER_RUN_DIR"' EXIT
export SEATA_HANDLER_WIRE_DIR="$HANDLER_RUN_DIR/handler-fixtures"

bash "$ROOT/tools/java-v1-golden/verify.sh"

git clone --filter=blob:none --no-checkout https://github.com/apache/incubator-seata-go.git "$HANDLER_RUN_DIR/seata-go"
git -C "$HANDLER_RUN_DIR/seata-go" checkout --detach 82570d905919c1c66bb025a3a5979f25390de498
GO_PROBE_DIR="$HANDLER_RUN_DIR/seata-go/tools/seata-rust-handler-wires"
mkdir -p "$GO_PROBE_DIR"
cp "$ROOT/tools/java-handler-wires/go_probe/main.go" "$GO_PROBE_DIR/main.go"
(
  cd "$GO_PROBE_DIR"
  GOTOOLCHAIN=local GOWORK=off go run . "$SEATA_HANDLER_WIRE_DIR"
)

SEATA_IT_HANDLER_WIRE_DIR="$SEATA_HANDLER_WIRE_DIR" \
  cargo test --manifest-path "$ROOT/tools/java-tc-integration/rust-client/Cargo.toml"
