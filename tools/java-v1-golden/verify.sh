#!/usr/bin/env bash
set -euo pipefail

# Overlay the Seata-Rust V1 frame tests onto the exact Java Seata source snapshot.
# Requirements: git, Maven, a JDK supported by the pinned Seata commit, and network access.

ROOT="$(git rev-parse --show-toplevel)"
JAVA_SHA="cba73e180fd8896a99df6415b859d275122feaa7"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git clone --filter=blob:none --no-checkout https://github.com/apache/incubator-seata.git "$TMP/seata-java"
git -C "$TMP/seata-java" checkout --detach "$JAVA_SHA"

TEST_DIR="$TMP/seata-java/test-suite/test-new-version/src/test/java/org/apache/seata/core/rpc/netty/v1"
mkdir -p "$TEST_DIR"
cp "$ROOT/tools/java-v1-golden/ProtocolV1GoldenFrameTest.java" "$TEST_DIR/ProtocolV1GoldenFrameTest.java"

(
  cd "$TMP/seata-java"
  mvn -pl test-suite/test-new-version -am \
    -Dtest=ProtocolV1GoldenFrameTest \
    -Dsurefire.failIfNoSpecifiedTests=false \
    test
)
