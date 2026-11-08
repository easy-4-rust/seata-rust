#!/usr/bin/env bash
set -euo pipefail

# Run the exact Seata-Go codec snapshot against the committed Seata-Rust fixtures.
# Requirements: git, Go toolchain, and network access for Go module dependencies.

ROOT="$(git rev-parse --show-toplevel)"
GO_SHA="82570d905919c1c66bb025a3a5979f25390de498"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git clone --filter=blob:none --no-checkout https://github.com/apache/incubator-seata-go.git "$TMP/seata-go"
git -C "$TMP/seata-go" checkout --detach "$GO_SHA"

HARNESS_DIR="$TMP/seata-go/tools/seata-rust-golden-vectors"
mkdir -p "$HARNESS_DIR"
cp "$ROOT/tools/go-golden-vectors/main.go" "$HARNESS_DIR/main.go"
cp "$ROOT/tools/go-golden-vectors/expected.json" "$HARNESS_DIR/expected.json"
cp "$ROOT/tools/go-golden-vectors/frames.json" "$HARNESS_DIR/frames.json"
gofmt -w "$HARNESS_DIR/main.go"

(
  cd "$TMP/seata-go"
  GOWORK=off go run ./tools/seata-rust-golden-vectors
)
