# ADR-0008: Preserve Wire-Level Optionality and Uncertain Transaction Outcomes

- **Status:** Proposed
- **Date:** 2026-11-08
- **Scope:** V1 protocol model, Rust TM API and error taxonomy

## Context

Phase 0 identified two behaviors that must not be flattened into generic strings or errors:

1. The pinned Java Seata V1 `GlobalBeginResponseCodec.decode` only assigns XID and ExtraData when the encoded string length is greater than zero. A zero-length field therefore remains `null` in the Java object. The pinned Seata-Go decoder represents the same wire value as an empty string / empty byte slice. Both encoders write zero length for these values, so the serialized bytes can be identical while the decoded object representations differ.
2. A transport timeout or connection loss after a request has been sent does not prove that TC rejected or failed the transaction. GlobalBegin may have created a session; GlobalCommit may have started commit or async cleanup; BranchRegister may have allocated a branch and lock.

Java Seata 2.x is the V1 compatibility authority. Seata-Go v2 is the primary implementation reference, but Go's in-memory representation must not override Java wire semantics.

## Decision

### 1. Keep wire optionality explicit

For fields whose Java V1 decoder distinguishes absent/unset from non-empty values, Rust protocol models must use an explicit optional representation rather than silently replacing null/unset with an empty value.

For GlobalBeginResponse:
- decode zero-length XID as `None`;
- decode zero-length ExtraData as `None`;
- decode non-empty fields as `Some(value)`;
- encode `None` and empty values according to the Java V1 zero-length encoding contract;
- do not claim object-level Java/Go equality when only serialized byte equality has been established.

If a higher-level Rust API prefers empty strings for ergonomics, provide that as an explicit adapter above the protocol model.

### 2. Separate definite rejection from unknown outcome

The public transaction error model must distinguish at least:

- **DefiniteRejected:** a decoded TC response explicitly rejects the operation.
- **DefiniteCompleted:** a decoded TC response provides a known result/status.
- **UnknownOutcome:** request may have reached TC, but no authoritative response was received.
- **LocalFailureBeforeSend:** request was not sent; no TC-side effect is implied.
- **ProtocolFailure:** malformed or unsupported protocol data; do not retry unchanged bytes.

A timeout after a successful write must not be mapped to DefiniteRejected.

### 3. Respect TC ownership of retries and retry-timeout states

For V1 (Rust TM/RM + Java TC):
- Java TC owns global retry scheduling and durable recovery.
- CommitRetryTimeout and RollbackRetryTimeout are authoritative TC states requiring operator intervention unless Java explicitly reports a transition.
- Rust TM must query/reconcile GlobalStatus after an unknown commit/rollback outcome.
- Rust TM must not start a competing local retry loop.
- Rust TM must not issue an automatic second GlobalBegin after an unknown begin outcome without an idempotency/correlation mechanism.

## Alternatives considered

1. **Normalize every zero-length field to empty string.** Rejected because it does not preserve Java decoder object semantics and can obscure nullable fields.
2. **Normalize every zero-length field to null.** Rejected as a universal rule because individual message codecs may have different contracts. Apply only where source proves the behavior.
3. **Map all timeouts to transaction failure.** Rejected because a request may have succeeded remotely and only its response was lost.
4. **Let Rust TM retry independently.** Rejected for V1 because Java TC owns the authoritative retry scheduler and session state.

## Consequences

- Rust protocol DTOs may contain `Option<T>` even when Go models use empty values.
- Error handling and transaction APIs become more expressive and require explicit matching.
- Unknown outcomes require status reconciliation and retaining XID/BranchId context.
- Tests must compare both wire bytes and decoded semantic fields.
- Retry-timeout states must remain visible to operators; they are not generic terminal success or automatic retry states.

## Evidence and validation gates

- [Java GlobalBeginResponseCodec](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/serializer/seata-serializer-seata/src/main/java/org/apache/seata/serializer/seata/protocol/transaction/GlobalBeginResponseCodec.java)
- [Go GlobalBeginResponseCodec](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/codec/global_begin_response_codec.go)
- [Java retry-timeout recovery audit](../phase-0/java-tc-retry-timeout-recovery.md)
- [Java V1 frame compatibility](../phase-0/protocol-v1-frame-compatibility.md)
- [TM/TC/RM uncertain-outcome matrix](../phase-0/tm-tc-rm-call-chains.md)

Acceptance requires:
- Java/Go/Rust codec tests for zero-length and non-empty XID/ExtraData.
- Java V1 frame decode/re-encode test for failed GlobalBegin response.
- TM tests for timeout before send, timeout after send, response lost, and status reconciliation.
- Java TC recovery tests REC-TIMEOUT-001–010 before defining any Rust TC retry-timeout lifecycle.
