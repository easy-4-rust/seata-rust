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

The full architecture plan, module-by-module comparison, transaction execution flows, compatibility matrix, risk register, testing strategy and phased OpenSpec roadmap are documented here:

**[Seata-Rust Architecture & Roadmap](docs/architecture/SEATA_RUST_ARCHITECTURE_PLAN.md)**

The document uses GitHub-rendered Markdown tables, Mermaid diagrams, code examples and task checklists for an at-a-glance, page-like reading experience.

## Reference Projects

- [Apache Seata Java 2.x](https://github.com/apache/incubator-seata/tree/2.x)
- [Apache Seata-Go v2](https://github.com/apache/incubator-seata-go)
- [RSeata](https://github.com/oulover/rseata)

## Project Status

Architecture planning. No production-readiness or protocol-conformance claim is made until implementation and compatibility tests provide evidence.

## License

Apache-2.0. Review upstream NOTICE, license headers and third-party dependency obligations before reusing upstream code or definitions.
