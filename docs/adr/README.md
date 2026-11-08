# Architecture Decision Records

Seata-Rust uses ADRs to keep architectural choices explicit, reviewable and traceable to the Phase 0 source baseline.

## Status definitions

- **Accepted** — current project direction; implementation must follow unless a superseding ADR is approved.
- **Proposed** — recommended design awaiting implementation evidence or explicit review.
- **Superseded** — replaced by a later ADR; retain for history.

## Decision index

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-v1-java-tc-first.md) | V1 uses Rust TM/RM with Java Seata TC | Accepted |
| [0002](0002-protocol-transport-separation.md) | Separate Seata message protocol from transport | Accepted |
| [0003](0003-explicit-wire-code-mapping.md) | Rust domain enums must use explicit Java wire-code mappings | Accepted |
| [0004](0004-v1-scope-at-mysql.md) | V1 acceptance starts with MySQL AT and Java TC interoperability | Accepted |
| [0005](0005-rseata-reuse-policy.md) | Reuse RSeata code only after semantic and protocol audit | Proposed |
| [0006](0006-tc-provider-boundary.md) | Hide Java/Rust TC choice behind a TC endpoint/provider boundary | Proposed |

## ADR template

New ADRs should contain:

1. Status and date.
2. Context and problem.
3. Decision.
4. Alternatives considered.
5. Consequences and risks.
6. Source evidence and validation required.

ADRs describe decisions; they do not substitute for detailed protocol, state-machine or module specifications.