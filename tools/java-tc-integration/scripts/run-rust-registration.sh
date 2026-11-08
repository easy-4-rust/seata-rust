#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(git rev-parse --show-toplevel)"
MANIFEST="$ROOT/tools/java-tc-integration/rust-client/Cargo.toml"

command -v cargo >/dev/null 2>&1 || {
  echo "cargo is required for the independent Rust registration client" >&2
  exit 2
}

cargo test --manifest-path "$MANIFEST"
cargo run --manifest-path "$MANIFEST" --bin seata-java-tc-it
