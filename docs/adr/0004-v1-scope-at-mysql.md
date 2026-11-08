# ADR-0004: V1 Acceptance Starts with MySQL AT

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

The first release must demonstrate that Rust TM/RM can safely participate in a real global transaction managed by Java Seata TC. The highest-value end-to-end path is AT because it exercises SQL interception, before/after images, branch registration, global locks, local ACID transaction boundaries, Undo Log persistence and phase-2 compensation.

Attempting MySQL, PostgreSQL, XA, TCC and Saga simultaneously would multiply correctness variables before the first protocol-compatible path is stable.

## Decision

V1's first end-to-end acceptance target is:

- Java Seata 2.x TC (pinned compatibility baseline).
- Rust TM and Rust RM.
- Native Seata V1 protocol, proven against Java TC.
- MySQL database.
- AT transaction mode.
- INSERT / UPDATE / DELETE support for the explicitly documented supported SQL subset.
- Correct branch register/report, global lock handling, phase-2 commit cleanup and rollback compensation.

XA, TCC, PostgreSQL and Saga are staged after the AT path has passed compatibility and failure-injection tests. They are not implicitly supported because an adapter trait or feature flag exists.

## Alternatives considered

- **All four transaction modes in V1:** rejected due to correctness and validation scope.
- **XA first:** rejected because it does not validate the AT-specific SQL image and Undo Log path that differentiates Seata's AT mode.
- **ORM-only abstraction:** rejected; a database adapter must define and test transaction boundaries and SQL execution semantics, not merely implement a generic ORM trait.

## Consequences

- V1 can be tested with a small, reproducible topology.
- The supported SQL subset must be documented; unsupported statements must fail safely.
- MySQL-specific image and SQL behavior is isolated behind adapter traits.
- Performance work is postponed until correctness is demonstrated.

## Validation required

- Atomic business row + Undo Log write.
- Global lock conflict prevents local business commit.
- Correct before/after image for supported INSERT/UPDATE/DELETE.
- Dirty-data validation and idempotent rollback.
- Duplicate commit/rollback and lost response handling.
- RM reconnect and TC retry behavior.
- Cross-language transaction involving Rust and Java services.

## Source evidence

- [Go AT connection](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/datasource/sql/conn_at.go)
- [Go AT transaction](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/datasource/sql/tx_at.go)
- [Go AT executor tests](https://github.com/apache/incubator-seata-go/tree/82570d905919c1c66bb025a3a5979f25390de498/pkg/datasource/sql/exec/at)
- [Java Undo Log manager](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/rm-datasource/src/main/java/org/apache/seata/rm/datasource/undo/AbstractUndoLogManager.java)