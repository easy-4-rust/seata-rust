# ADR-0006: Hide the TC Choice Behind a Coordinator Endpoint Boundary

- **Status:** Proposed
- **Date:** 2026-11-08

## Context

V1 uses Java Seata TC. A later phase may introduce a Rust-native TC. If TM/RM business code directly depends on Java-specific connection and server assumptions, switching TC implementations will require rewriting the client and database adapters.

However, Java and Rust TC implementations may not be interchangeable until Rust TC implements the same message contract, state semantics, lock behavior, recovery guarantees and operational behavior.

## Decision

Introduce a coordinator endpoint/provider boundary that separates:

- transaction operations (begin, commit, rollback, status, report);
- branch operations (register, report, lock query);
- resource registration and phase-2 instruction delivery;
- transport and protocol version negotiation.

The V1 provider speaks the Java Seata native protocol. A future Rust TC provider may be selected only after it passes the same conformance suite.

The boundary must not hide semantic differences behind a generic RPC trait. Capability and protocol-version negotiation must be explicit.

## Alternatives considered

- **Hard-code Java TC into TM/RM:** rejected because it creates avoidable coupling.
- **Assume any gRPC TC implements Seata:** rejected; RSeata's custom proto is not the Java wire contract.
- **Make all TC implementations identical by trait alone:** rejected; a Rust trait cannot guarantee wire or transaction semantic compatibility.

## Consequences

- V1 can be implemented without pre-implementing Rust TC.
- V2 can introduce a Rust TC behind the same client-facing boundary.
- Compatibility is enforced by protocol conformance and behavior tests, not type signatures alone.
- The provider interface must be kept small and must not leak transport-specific message objects into TM/RM.

## Validation required

- Run the same TM/RM conformance suite against Java TC and future Rust TC.
- Prove Java and Go clients can use Rust TC before declaring multi-language compatibility.
- Verify recovery, locking and retry semantics, not only RPC success.

## Related decisions

- [ADR-0001: V1 Java TC first](0001-v1-java-tc-first.md)
- [ADR-0002: Protocol/transport separation](0002-protocol-transport-separation.md)
- [ADR-0003: Explicit wire-code mapping](0003-explicit-wire-code-mapping.md)