# ADR-0003: Use Explicit Wire-Code Mapping for Transaction Enums

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Java Seata, Seata-Go and RSeata do not share one consistent numeric representation for every transaction enum.

Confirmed examples from the pinned sources:

- Java/Go BranchType uses AT=0, TCC=1, SAGA=2, XA=3; Java also has SAGA_ANNOTATION.
- RSeata internal BranchType uses 1..4, while its proto enum uses 0..3. Its `From<i32>` conversion maps 1 to AT and defaults unknown values to AT.
- Java/Go GlobalStatus uses AsyncCommitting=8 and Committed=9. RSeata internal status follows this, but its proto enum omits AsyncCommitting and assigns Committed=8.
- Java BranchStatus contains codes 0..15. Go's branch model exposes a smaller set. RSeata internal BranchStatus and proto enum use different orderings.

Direct enum casts can silently change transaction meaning.

## Decision

- Rust domain enums are not wire enums.
- Every protocol boundary uses explicit, exhaustive conversion functions.
- Unknown or unsupported wire codes return a protocol error; they must never default to AT, Unknown or another business mode unless the upstream contract explicitly defines that behavior.
- Preserve the upstream numeric code even when the Rust enum declaration order differs.
- Mode-specific states remain separate from global/branch protocol states.
- Protocol version adapters own compatibility mapping; domain code must not contain version-specific numeric hacks.

Example design:

```rust
pub fn branch_type_from_java_v1(code: i32) -> Result<BranchType, ProtocolError> {
    match code {
        0 => Ok(BranchType::At),
        1 => Ok(BranchType::Tcc),
        2 => Ok(BranchType::Saga),
        3 => Ok(BranchType::Xa),
        _ => Err(ProtocolError::UnknownBranchType(code)),
    }
}
```

The example is illustrative; exact Java version support and SAGA_ANNOTATION handling must be finalized from the selected server baseline.

## Alternatives considered

- **Use Rust enum discriminants as wire values:** rejected; Rust declaration order must not become a compatibility contract.
- **Reuse RSeata proto enums directly:** rejected; their current numeric values do not match Java/Go status semantics.
- **Map unknown BranchType to AT:** rejected as unsafe and semantically lossy.

## Consequences

- Slightly more conversion code and explicit version mapping.
- Protocol changes become testable and reviewable.
- Prevents a wrong branch mode or wrong transaction status from being silently accepted.
- Requires exhaustive table-driven tests for all supported codes and unknown values.

## Validation required

- Test all Java GlobalStatus codes 0..20.
- Test all Java BranchStatus codes 0..15.
- Test supported BranchType codes and unsupported SAGA_ANNOTATION policy.
- Test round-trip encode/decode against Go and Java golden vectors.
- Test rejection of unknown values.

## Source evidence

- [Java GlobalStatus](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/common/src/main/java/org/apache/seata/core/model/GlobalStatus.java)
- [Java BranchStatus](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/model/BranchStatus.java)
- [Java BranchType](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/model/BranchType.java)
- [Go branch model](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/branch/branch.go)
- [RSeata branch types](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/branch/mod.rs)
- [RSeata proto enums](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-proto/proto/rseata.proto)