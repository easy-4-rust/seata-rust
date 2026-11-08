# ADR-0001: V1 Uses Rust TM/RM with Java Seata TC

- **Status:** Accepted
- **Date:** 2026-11-08
- **Decision owners:** Seata-Rust maintainers

## Context

Implementing a production-grade TC requires global/branch sessions, global locks, persistence, retry scheduling, timeout handling, recovery, resource registration and cluster semantics. Reimplementing all of this before a working Rust client exists would delay the first usable release and duplicate mature Java Seata functionality.

Seata-Go provides a directly relevant architecture precedent: its README states that TC functionality is reused from Java Seata while Go implements TM/RM and connects to Java Seata. The project explicitly targets Java/Go interoperability.

The project owner has selected this staged strategy: first follow the Seata-Go model, then consider implementing the complete Java Seata 2.x feature set in Rust.

## Decision

V1 consists of:

- Rust TM.
- Rust RM.
- Rust transaction context and XID propagation.
- Java Seata 2.x TC as the coordinator.
- Java-compatible native Seata protocol and message semantics.
- MySQL AT as the first end-to-end acceptance mode.

The Rust client must not implement an independent global transaction coordinator in V1. It must use the XID and Branch ID issued by TC and obey TC's global and branch state outcomes.

XA and TCC are later V1.x increments after AT's protocol and failure semantics are validated. Saga is not a V1 requirement.

## Alternatives considered

1. **Implement Rust TC first.** Rejected because it expands the first milestone to persistence, lock management, retry/recovery and cluster correctness before the client has demonstrated Java compatibility.
2. **Use RSeata's existing custom Tonic TC as V1.** Rejected as the primary V1 route because its custom Protobuf API is not the Java native Seata remoting protocol.
3. **Fork Java TC or run a permanent Java sidecar for every Rust service.** Rejected; V1 reuses the normal Java Seata Server deployment rather than adding a per-service JVM dependency.

## Consequences

Positive:
- First useful Rust client can arrive earlier.
- Java TC remains the single source of global transaction truth in V1.
- Rust TM/RM work becomes the foundation for a later Rust TC.

Costs and constraints:
- V1 requires exact Java Seata protocol compatibility.
- V1 cannot claim independent operation without Java TC.
- Rust RM must correctly process second-phase instructions and retries.
- TC version compatibility must be tested against pinned Java server versions.

## Acceptance evidence required

- Rust TM begins, commits and rolls back a global transaction through the pinned Java TC.
- Rust RM registers and reports branches.
- Java and Rust services participate in the same global transaction.
- AT MySQL commit/rollback and lock-conflict tests pass.
- Network loss, duplicate second-phase request and RM reconnect tests pass.

## Source evidence

- [Seata-Go README at pinned commit](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/README.md)
- [Seata-Go TM executor](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/tm/transaction_executor.go)
- [Seata-Go RM API](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/rm/rm_api.go)