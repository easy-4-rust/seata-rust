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

## Source snapshots

| Source | Ref used | Commit | Snapshot date | Role |
|---|---|---|---|---|
| [Apache Seata Java](https://github.com/apache/incubator-seata/tree/cba73e180fd8896a99df6415b859d275122feaa7) | `2.x` | `cba73e180fd8896a99df6415b859d275122feaa7` | 2026-10-31 | Protocol and behavior baseline |
| [Apache Seata-Go](https://github.com/apache/incubator-seata-go/tree/82570d905919c1c66bb025a3a5979f25390de498) | `master` | `82570d905919c1c66bb025a3a5979f25390de498` | 2026-11-07 | V1 client architecture and cross-language reference |
| [RSeata](https://github.com/oulover/rseata/tree/c6062776c72a2490f62f8ebb25012c60884f62bd) | `master` | `c6062776c72a2490f62f8ebb25012c60884f62bd` | 2026-03-10 | Existing Rust code audit source |

**Pinning rule:** documents must cite commit-pinned paths, not moving `master` / `2.x` links alone. When an upstream snapshot changes, update the source manifest and record what was re-evaluated.

## Phase 0 documents

| Document | Purpose |
|---|---|
| [Source Mapping](source-mapping.md) | File-level source map across Java, Go and RSeata; proposed Rust module ownership |
| [Protocol Compatibility](protocol-compatibility.md) | Wire framing, message types, payload contracts, XID and transport boundaries |
| [Transaction State Machines](transaction-state-machines.md) | Global/branch states, status code comparison, transition semantics and known mismatches |
| [ADR index](../adr/README.md) | Accepted and proposed architecture decisions |

## Current high-impact findings

1. Java Seata 2.x defines protocol versions 0, 1 and 2, and the current `ProtocolConstants.VERSION` is 2. Seata-Go's native protocol package declares version 1 and a 16-byte V1 fixed header. V1 compatibility with the selected Java 2.x server must be demonstrated through integration tests; it must not be assumed from matching magic bytes alone.
2. Seata-Go supports both `seata` and `grpc` transport selections. Its native Seata message codec is a separate concern from transport selection.
3. RSeata's `rseata.proto` defines an independent gRPC service contract. It is not the Java Seata native remoting wire protocol.
4. RSeata's current XID construction (`hex(transaction_id):transaction_service_group`) differs from Java Seata's `ip:port:transaction_id` form. V1 must accept the XID returned by TC rather than minting an independent format.
5. RSeata's internal `BranchType` numeric values and its Protobuf enum values do not align with Java/Go ordinal values. Its Protobuf global and branch status enums also omit/reorder values relative to the Java status model. These are protocol correctness risks, not cosmetic naming differences.
6. RSeata exposes an in-memory session store implementation. Its `read_session_by_session_condition` path currently returns all sessions in the inspected implementation; this is not evidence of a durable recovery store.
7. RSeata's `DefaultGlobalSession::is_timeout` calculation compares elapsed time to `begin_time_millis * 1000`, despite storing `timeout_millis`. This requires correction before any code reuse in a coordinator.

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
- [ ] Java TC V1 protocol acceptance is confirmed against the pinned server with golden byte vectors and a live integration test.
- [ ] XID, BranchType, BranchStatus and GlobalStatus mappings are explicit and tested.
- [ ] Global and branch state transitions have source references and an allowed-transition table.
- [ ] TC/TM/RM execution paths are mapped down to handler and resource adapter entry points.
- [ ] RSeata reuse candidates have been classified as keep/refactor/rewrite with test evidence.
- [ ] ADRs are reviewed and the V1 scope is frozen.
- [ ] V1 OpenSpec tasks are derived from the approved specifications.

**Non-goal:** Phase 0 does not implement TM, RM, AT, XA, TCC, Saga or a Rust TC.