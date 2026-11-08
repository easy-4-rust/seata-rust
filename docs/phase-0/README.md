# Phase 0 — Architecture & Compatibility Baseline

> **Status: In progress · Research/specification only**
>
> Phase 0 establishes source provenance, protocol facts, state-machine semantics and architecture decisions before implementation begins. No application/business code is authorized by this phase.

## Goals

1. Pin the exact upstream source snapshots used as compatibility references.
2. Map Java Seata 2.x, Seata-Go v2 and RSeata source entry points to Seata-Rust modules.
3. Specify the wire protocol and identify what is genuinely interoperable.
4. Record global/branch transaction states and the semantic gaps between implementations.
5. Make the V1 architecture decisions explicit and reviewable.
6. Turn source-derived protocol vectors into reproducible Go encoder and Java decoder tests.

## Source snapshots

| Source | Ref used | Commit | Snapshot date | Role |
|---|---|---|---|---|
| [Apache Seata Java](https://github.com/apache/incubator-seata/tree/cba73e180fd8896a99df6415b859d275122feaa7) | `2.x` | `cba73e180fd8896a99df6415b859d275122feaa7` | 2026-10-31 | Protocol and behavior baseline |
| [Apache Seata-Go](https://github.com/apache/incubator-seata-go/tree/82570d905919c1c66bb025a3a5979f25390de498) | `master` | `82570d905919c1c66bb025a3a5979f25390de498` | 2026-11-07 | V1 client architecture and cross-language reference |
| [RSeata](https://github.com/oulover/rseata/tree/c6062776c72a2490f62f8ebb25012c60884f62bd) | `master` | `c6062776c72a2490f62f8ebb25012c60884f62bd` | 2026-03-10 | Independent third-party Rust reference only |

**Pinning rule:** documents must cite commit-pinned paths, not moving `master` / `2.x` links alone. When an upstream snapshot changes, update the source manifest and record what was re-evaluated.

## Phase 0 documents

| Document | Purpose |
|---|---|
| [Source Mapping](source-mapping.md) | File-level source map across Java, Go and RSeata; proposed Rust module ownership |
| [Protocol Compatibility](protocol-compatibility.md) | Wire framing, message types, payload contracts, XID and transport boundaries |
| [Transaction State Baseline](transaction-state-machines.md) | Global/branch state codes and cross-language semantic hazards |
| [Java TC Transition & Recovery Matrix](java-tc-state-transition-matrix.md) | Source-derived state transitions, retries, timeouts, terminal states and restart recovery |
| [V1 Payload Golden Vectors](protocol-golden-vectors.md) | Deterministic codec-body and serialized-payload hex vectors, Go verification harness and certification gates |
| [Java V1 Frame Compatibility](protocol-v1-frame-compatibility.md) | Java V1 encoder/decoder source audit, fixed frame layout and runtime interop gates |
| [Go codec/frame harness](../../tools/go-golden-vectors/verify.sh) | Execute 25 pinned Seata-Go codec vectors, serializer type prefixes and two complete V1 frame fixtures |
| [Java V1 frame harness](../../tools/java-v1-golden/verify.sh) | Execute Java V1 decoder → encoder exact-frame round trips against the pinned Java source snapshot |
| [TM/TC/RM Call Chains](tm-tc-rm-call-chains.md) | Method-level execution paths and uncertain-outcome/error matrix |
| [RSeata Module Disposition](rseata-module-disposition.md) | Keep/Refactor/Rewrite classification of third-party RSeata modules |
| [ADR index](../adr/README.md) | Accepted and proposed architecture decisions |

## Current high-impact findings

1. Java Seata 2.x defines protocol versions 0, 1 and 2, and the current `ProtocolConstants.VERSION` is 2. Seata-Go's native protocol package declares version 1 and a 16-byte V1 fixed header. V1 compatibility with the selected Java 2.x server must be demonstrated through integration tests; it must not be assumed from matching magic bytes alone.
2. Seata-Go supports both `seata` and `grpc` transport selections. Its native Seata message codec is a separate concern from transport selection.
3. The pinned Java Seata snapshot contains V1 `ProtocolDecoderV1`, `ProtocolEncoderV1` and `SeataSerializerV1` implementations. V1 support exists in source, but the default protocol version is 2; runtime protocol selection and actual TC acceptance remain to be tested.
4. The Go `CodecManager.Encode` and Java `SeataSerializerV1.serialize` both prepend a big-endian uint16 message type to the message-specific codec body. The golden-vector document now distinguishes codec body, serialized message payload and full TCP frame.
5. RSeata's `rseata.proto` defines an independent gRPC service contract. It is not the Java Seata native remoting wire protocol.
6. RSeata's current XID construction (`hex(transaction_id):transaction_service_group`) differs from Java Seata's `ip:port:transaction_id` form. V1 must accept the XID returned by TC rather than minting an independent format.
7. RSeata's internal `BranchType` numeric values and its Protobuf enum values do not align with Java/Go ordinal values. Its Protobuf global and branch status enums also omit/reorder values relative to the Java status model. These are protocol correctness risks, not cosmetic naming differences.
8. RSeata exposes an in-memory session store implementation. Its `read_session_by_session_condition` path currently returns all sessions in the inspected implementation; this is not evidence of a durable recovery store.
9. RSeata's `DefaultGlobalSession::is_timeout` calculation compares elapsed time to `begin_time_millis * 1000`, despite storing `timeout_millis`. This is a defect in the third-party reference snapshot and must not be copied.

## Evidence labels

Use these labels consistently:

- **Verified in source** — directly visible in a pinned source file.
- **Verified by test** — a reproducible test has passed and its command/result is recorded.
- **Inferred** — a reasoned interpretation that still needs a targeted test or deeper source tracing.
- **Unknown** — not yet established; do not implement from assumption.

A directory, type declaration, Cargo feature or README claim is not proof of working behavior.

## Exit criteria

Phase 0 is complete only when:

- [ ] All V1-required protocol message payloads have a Java/Go field-by-field mapping.
- [ ] Every candidate payload vector is generated by the pinned Go encoder, cross-decoded by Java and Rust, and stored as a binary fixture with provenance.
- [ ] Java TC V1 protocol acceptance is confirmed against the pinned server with golden byte vectors and a live integration test.
- [ ] `bash tools/go-golden-vectors/verify.sh` passes against the pinned Go commit.
- [ ] Java `ProtocolDecoderV1` accepts the committed complete V1 frames and Java `ProtocolEncoderV1` emits byte-identical frames for the fixtures.
- [ ] XID, BranchType, BranchStatus and GlobalStatus mappings are explicit and tested.
- [ ] Global and branch state transitions have source references and an allowed-transition table; the remaining transition call sites and mode-specific exceptions are closed.
- [ ] TC/TM/RM execution paths are mapped down to handler and resource adapter entry points.
- [ ] RSeata reference modules have a Keep/Refactor/Rewrite disposition; any actual source reuse has separate license and test evidence.
- [ ] ADRs are reviewed and the V1 scope is frozen.
- [ ] V1 OpenSpec tasks are derived from the approved specifications.

**Reference-role clarification:** RSeata is an independent third-party Rust project, not the user's implementation and not the Seata-Rust migration baseline. Seata-Go v2 is the primary V1 engineering reference; Java Seata 2.x remains the compatibility authority. The payload vectors are currently source-derived candidates, not Go-runtime, Java-interoperability or Rust-conformance test results.

**Validation status:** the Go harness (29 payloads + 2 frames) and Java harness (2 full-frame decoder/encoder round trips) are committed but have not been executed in this session. The Go toolchain is available in this container, but the pinned upstream checkout cannot be fetched from this network-restricted execution environment; Maven is not installed. Java V1 support is verified in source only; runtime acceptance remains unknown.

**Non-goal:** Phase 0 does not implement TM, RM, AT, XA, TCC, Saga or a Rust TC.