# Seata-Rust

![Rust](https://img.shields.io/badge/language-Rust-orange?logo=rust)
![License](https://img.shields.io/badge/license-Apache--2.0-blue)
![Roadmap](https://img.shields.io/badge/roadmap-V1%20Client%20%E2%86%92%20Rust%20TC-success)

**Rust-native distributed transaction infrastructure in the Apache Seata ecosystem.**

Seata-Rust follows a staged strategy:

1. **V1 — Java TC compatibility:** implement Rust TM/RM and protocol compatibility, reusing the mature Java Seata Transaction Coordinator.
2. **V2 — Native Rust TC:** progressively implement transaction coordination, persistence, recovery, locking and cluster capabilities in Rust.
3. **Long term — Seata ecosystem coverage:** expand transaction modes, database/ORM adapters, discovery, observability and multi-language interoperability.

## Architecture & Roadmap

- [Seata-Rust Architecture & Roadmap](docs/architecture/SEATA_RUST_ARCHITECTURE_PLAN.md) — overall vision, module matrix, execution flows, staged roadmap and V1 acceptance goals.
- [Phase 0 Research Index](docs/phase-0/README.md) — pinned upstream snapshots and all Phase 0 deliverables.
- [Java TC State Transition Matrix](docs/phase-0/java-tc-state-transition-matrix.md) — global/branch states, retry, timeout and recovery.
- [V1 Payload Golden Vectors](docs/phase-0/protocol-golden-vectors.md) — source-derived byte vectors with explicit certification status.
- [TM/TC/RM Call Chains](docs/phase-0/tm-tc-rm-call-chains.md) — method-level flows and failure matrix.
- [RSeata Reference Disposition](docs/phase-0/rseata-module-disposition.md) — third-party Rust reference audit, not a migration plan.
- [Architecture Decision Records](docs/adr/README.md) — accepted and proposed architecture decisions.

### Phase 0 scope

Phase 0 is research and specification only. It establishes source provenance, protocol facts, state semantics, compatibility risks and ADRs before implementation begins. **No application/business code is being started in this phase.**

The Phase 0 documents distinguish source-verified facts, test-verified behavior, inference and unknowns. A README claim, source directory, Cargo feature or matching enum name is not considered proof of compatibility.

## Reference Projects

- [Apache Seata Java 2.x](https://github.com/apache/incubator-seata/tree/2.x)
- [Apache Seata-Go v2](https://github.com/apache/incubator-seata-go)
- [RSeata](https://github.com/oulover/rseata)

## Project Status

Architecture planning / Phase 0 research. No production-readiness or protocol-conformance claim is made until implementation and compatibility tests provide evidence.

## License

Apache-2.0. Review upstream NOTICE, license headers and third-party dependency obligations before reusing upstream code or definitions.
