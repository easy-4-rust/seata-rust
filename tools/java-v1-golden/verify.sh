#!/usr/bin/env bash
set -euo pipefail

# Overlay the Seata-Rust V1 frame and TC exception tests onto the exact Java Seata source snapshot.
# Requirements: git, Maven, Python3, a JDK supported by the pinned Seata commit, and network access.

ROOT="$(git rev-parse --show-toplevel)"
JAVA_SHA="cba73e180fd8896a99df6415b859d275122feaa7"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git clone --filter=blob:none --no-checkout https://github.com/apache/incubator-seata.git "$TMP/seata-java"
git -C "$TMP/seata-java" checkout --detach "$JAVA_SHA"

TEST_DIR="$TMP/seata-java/test-suite/test-new-version/src/test/java/org/apache/seata/core/rpc/netty/v1"
mkdir -p "$TEST_DIR"
cp "$ROOT/tools/java-v1-golden/ProtocolV1GoldenFrameTest.java" "$TEST_DIR/ProtocolV1GoldenFrameTest.java"

EXCEPTION_TEST_DIR="$TMP/seata-java/test-suite/test-new-version/src/test/java/org/apache/seata/server"
mkdir -p "$EXCEPTION_TEST_DIR"
cp "$ROOT/tools/java-v1-golden/AbstractTCInboundHandlerExceptionTest.java" "$EXCEPTION_TEST_DIR/AbstractTCInboundHandlerExceptionTest.java"
cp "$ROOT/tools/java-v1-golden/SessionHolderRetryTimeoutRecoveryTest.java" "$EXCEPTION_TEST_DIR/SessionHolderRetryTimeoutRecoveryTest.java"
cp "$ROOT/tools/java-v1-golden/FileRetryTimeoutStoreProbe.java" "$EXCEPTION_TEST_DIR/FileRetryTimeoutStoreProbe.java"
cp "$ROOT/tools/java-v1-golden/FileRetryTimeoutProcessTest.java" "$EXCEPTION_TEST_DIR/FileRetryTimeoutProcessTest.java"
cp "$ROOT/tools/java-v1-golden/BackendRetryTimeoutStoreProbe.java" "$EXCEPTION_TEST_DIR/BackendRetryTimeoutStoreProbe.java"
cp "$ROOT/tools/java-v1-golden/BackendRetryTimeoutProbeTest.java" "$EXCEPTION_TEST_DIR/BackendRetryTimeoutProbeTest.java"
cp "$ROOT/tools/java-v1-golden/TCInboundFailureMatrixTest.java" "$EXCEPTION_TEST_DIR/TCInboundFailureMatrixTest.java"
cp "$ROOT/tools/java-v1-golden/Phase0CredentialCheckAuthHandler.java" "$EXCEPTION_TEST_DIR/Phase0CredentialCheckAuthHandler.java"
cp "$ROOT/tools/java-v1-golden/Phase0CredentialCheckAuthHandlerTest.java" "$EXCEPTION_TEST_DIR/Phase0CredentialCheckAuthHandlerTest.java"
cp "$ROOT/tools/java-v1-golden/RaftRetryTimeoutControl.java" "$EXCEPTION_TEST_DIR/RaftRetryTimeoutControl.java"
cp "$ROOT/tools/java-v1-golden/RaftRetryTimeoutControlTest.java" "$EXCEPTION_TEST_DIR/RaftRetryTimeoutControlTest.java"
cp "$ROOT/tools/java-v1-golden/ProtocolV1ControlEnvelopeTest.java" "$EXCEPTION_TEST_DIR/ProtocolV1ControlEnvelopeTest.java"
cp "$ROOT/tools/java-v1-golden/GoErrorLengthCompatibilityTest.java" "$EXCEPTION_TEST_DIR/GoErrorLengthCompatibilityTest.java"
cp "$ROOT/tools/java-v1-golden/TransactionExceptionProfileTest.java" "$EXCEPTION_TEST_DIR/TransactionExceptionProfileTest.java"
cp "$ROOT/tools/java-v1-golden/MergedMessageCompatibilityTest.java" "$EXCEPTION_TEST_DIR/MergedMessageCompatibilityTest.java"

AUTH_TEST_DIR="$TMP/seata-java/test-suite/test-new-version/src/test/java/org/apache/seata/core/rpc/processor/server"
mkdir -p "$AUTH_TEST_DIR"
cp "$ROOT/tools/java-v1-golden/RegistrationProcessorAuthTest.java" "$AUTH_TEST_DIR/RegistrationProcessorAuthTest.java"

PAYLOAD_TEST_DIR="$TMP/seata-java/test-suite/test-new-version/src/test/java/org/apache/seata/serializer/seata"
mkdir -p "$PAYLOAD_TEST_DIR"
cp "$ROOT/tools/java-v1-golden/HandlerWireRecord.java" "$PAYLOAD_TEST_DIR/HandlerWireRecord.java"
cp "$ROOT/tools/java-v1-golden/PayloadGoldenCorpusTest.java" "$PAYLOAD_TEST_DIR/PayloadGoldenCorpusTest.java"
cp "$ROOT/tools/java-v1-golden/PayloadOptionalityTest.java" "$PAYLOAD_TEST_DIR/PayloadOptionalityTest.java"
cp "$ROOT/tools/java-v1-golden/FailureVariantCompatibilityTest.java" "$PAYLOAD_TEST_DIR/FailureVariantCompatibilityTest.java"
cp "$ROOT/tools/go-golden-vectors/expected.json" "$TMP/seata-java/test-suite/test-new-version/src/test/resources/seata-rust-payload-vectors.json"
cp "$ROOT/tools/go-failure-variants/expected.json" "$TMP/seata-java/test-suite/test-new-version/src/test/resources/seata-rust-failure-variants.json"

SELECTED_TESTS="PayloadGoldenCorpusTest,PayloadOptionalityTest,FailureVariantCompatibilityTest,ProtocolV1GoldenFrameTest,AbstractTCInboundHandlerExceptionTest,TCInboundFailureMatrixTest,RegistrationProcessorAuthTest,SessionHolderRetryTimeoutRecoveryTest,FileRetryTimeoutProcessTest,BackendRetryTimeoutProbeTest,Phase0CredentialCheckAuthHandlerTest,ProtocolV1ControlEnvelopeTest,GoErrorLengthCompatibilityTest,TransactionExceptionProfileTest,MergedMessageCompatibilityTest"
# Drive ownership guards belong to the dedicated Windows RAFT runner; Linux CI
# still compiles the helper but must not pretend it has D:/E: runtime paths.
if [[ "${OS:-}" == "Windows_NT" ]]; then
  SELECTED_TESTS+=",RaftRetryTimeoutControlTest"
fi

(
  cd "$TMP/seata-java"
  mvn -pl test-suite/test-new-version -am \
    "-Dtest=$SELECTED_TESTS" \
    -Dsurefire.failIfNoSpecifiedTests=false \
    test
) 2>&1 | tee "$TMP/java-handler-runtime.log"

HANDLER_CAPTURE_DIR="${SEATA_HANDLER_WIRE_DIR:-$TMP/handler-fixtures}"
python3 "$ROOT/tools/java-handler-wires/collect.py" \
  --log "$TMP/java-handler-runtime.log" --destination "$HANDLER_CAPTURE_DIR"
