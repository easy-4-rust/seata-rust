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
  cd "$HARNESS_DIR"
  GOWORK=off go run .
)

# Use the original native Getty path; missing merge codecs remain an OPEN
# diagnostic instead of registering a substitute codec in the reference.
CONTROL_DIR="$TMP/seata-go/tools/seata-rust-control-envelopes"
mkdir -p "$CONTROL_DIR"
cp "$ROOT/tools/go-control-envelopes/main.go" "$CONTROL_DIR/main.go"
(
  cd "$CONTROL_DIR"
  GOWORK=off go run . "$ROOT/tests/fixtures/protocol/v1/control_envelopes/vectors.json"
)

# Separate diagnostic matrix: known incompatibilities stay explicitly OPEN.
FAILURE_DIR="$TMP/seata-go/tools/seata-rust-failure-variants"
mkdir -p "$FAILURE_DIR"
cp "$ROOT/tools/go-failure-variants/main.go" "$FAILURE_DIR/main.go"
cp "$ROOT/tools/go-failure-variants/expected.json" "$FAILURE_DIR/expected.json"
gofmt -w "$FAILURE_DIR/main.go"
(
  cd "$FAILURE_DIR"
  GOWORK=off go run .
)
