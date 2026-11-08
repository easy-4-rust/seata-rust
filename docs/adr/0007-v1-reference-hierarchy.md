# ADR-0007: Seata-Go v2 Is the V1 Engineering Reference; RSeata Is External Reference Only

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Seata-Rust has an existing architecture plan with a staged direction:

- V1: Rust TM/RM interoperating with Java Seata TC, following the Seata-Go implementation model.
- Later phases: expand transaction modes and build a Rust-native TC.
- Final goal: broad Java Seata 2.x capability and protocol interoperability.

RSeata (`oulover/rseata`) is an independent third-party Rust project. It is not authored by the Seata-Rust project owner, is not a predecessor of this repository, and is not the source tree being migrated. It already contains Rust code that can help evaluate asynchronous architecture, traits, session abstractions, locks, gRPC and SeaORM adapters.

## Decision

The reference hierarchy is:

1. **Java Seata 2.x — compatibility and semantic authority.** Use it to establish protocol contracts, status codes, TC decisions, retry/recovery behavior and transaction-mode semantics.
2. **Seata-Go v2 — primary V1 engineering reference.** Use it to understand how a non-Java language implements TM/RM, context propagation, protocol codecs, AT/XA/TCC/Saga resource execution and Java TC interoperability.
3. **RSeata — third-party Rust design reference only.** Study its Rust abstractions and adapter experiments. Do not assume its code, wire protocol, status mappings or production behavior are correct or directly reusable.

The V1 architecture must be derived from the existing Seata-Rust architecture plan and verified against Seata-Go/Java source. RSeata must not redefine the V1 module layout or implementation order.

## Consequences

- RSeata findings are recorded as Keep/Refactor/Rewrite **recommendations for a third-party reference**, not as a migration backlog.
- A useful RSeata abstraction can inspire a fresh implementation without copying its code.
- Any actual code reuse requires separate license/NOTICE review, semantic comparison and tests.
- Seata-Go features are not automatically considered production-ready merely because the Go implementation has code for them; Rust acceptance still requires cross-language integration and failure tests.
- Java server behavior remains authoritative when Go and Java differ.

## Related documents

- [Architecture plan](../architecture/SEATA_RUST_ARCHITECTURE_PLAN.md)
- [Phase 0 source mapping](../phase-0/source-mapping.md)
- [RSeata reference disposition](../phase-0/rseata-module-disposition.md)
- [TM/TC/RM call chains](../phase-0/tm-tc-rm-call-chains.md)
- [ADR-0001: V1 Java TC first](0001-v1-java-tc-first.md)
