# ADR-0005: Treat RSeata as a Third-Party Rust Reference, Not the V1 Codebase

- **Status:** Proposed
- **Date:** 2026-11-08

## Context

RSeata is an independent third-party Rust project. It is not the user's implementation, is not a predecessor of Seata-Rust, and is not the V1 migration baseline. It is useful as a Rust design sample: it contains TC/TM/RM boundaries, Session models, Tonic services, lock abstractions and SeaORM AT/XA adapter experiments. The inspected snapshot also contains protocol and correctness gaps:

- Custom Protobuf service contract rather than Java native Seata remoting.
- XID generation differs from Java Seata.
- BranchType and BranchStatus numeric conversions are inconsistent.
- GlobalStatus Protobuf enum does not preserve the Java/Go code table.
- Memory-only transaction store implementation is present.
- SessionCondition query currently returns all sessions.
- Global session timeout calculation appears to use the wrong field.

Therefore, copying RSeata modules wholesale would import unverified semantics and known compatibility hazards.

## Decision

No RSeata module is accepted for direct production reuse solely because it compiles or has a matching name. Architecture inspiration and source-code reuse are separate decisions.

Every reuse candidate must pass:

1. Source-level semantic comparison with Java Seata 2.x and Seata-Go v2.
2. Protocol mapping audit.
3. Unit and integration tests against the pinned reference.
4. Failure-injection tests for lifecycle-sensitive code.
5. License and NOTICE review.

Classify each candidate as:

- **Keep** — behavior and contract match; tests prove it.
- **Refactor** — useful Rust structure, but semantics or abstraction need correction.
- **Rewrite** — protocol/state/persistence behavior is incompatible or unsafe.
- **Reject** — no clear value or introduces unacceptable coupling.

Initial disposition:
- Keep only after validation: XID wrapper, common error/result abstractions, tracing scaffolding.
- Refactor: Session model, coordinator interfaces, lock traits, SeaORM adapters.
- Rewrite: native Java protocol codec, XID generation/handling boundary, status mapping, durable TC store and recovery.
- Do not use in V1: custom RSeata TC as a substitute for Java-compatible TC.

## Alternatives considered

- **Fork RSeata and patch forward:** rejected as the default strategy because V1 has a different compatibility target and the existing service contract is custom.
- **Discard all RSeata code:** not selected; its Rust abstractions and adapter experiments can still inform design after review.
- **Copy then test later:** rejected; transaction correctness and wire compatibility must be established before reuse.

## Consequences

- Some code may be rewritten even if the current implementation compiles.
- The audit must preserve provenance and record reused source files.
- The project may retain RSeata-derived ideas without retaining its protocol or state model.

## Validation required

- A reuse ledger with source path, upstream commit, license, decision, tests and reviewer.
- No direct casts between RSeata Protobuf enums and internal enums.
- Durable-store recovery tests before any TC code is considered production-ready.
- AT/XA adapters pass Java-compatible transaction tests.

## Source evidence

- [RSeata branch model](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/branch/mod.rs)
- [RSeata global session](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/session/defaults/default_global_session.rs)
- [RSeata transaction store trait](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/store/transaction_store_manager.rs)
- [RSeata memory store](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/store/memery_transaction_store_manager.rs)
- [RSeata Protobuf contract](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-proto/proto/rseata.proto)