# ADR-0002: Separate Seata Message Protocol from Transport

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Java Seata 2.x has native remoting frame versions 0, 1 and 2. The current Java protocol default is version 2, while Seata-Go's native protocol package declares version 1 and also exposes a gRPC protocol option.

RSeata defines a separate Tonic/Protobuf service contract. That custom service API does not make it wire-compatible with Java Seata native remoting.

If the Rust client binds message encoding, transaction semantics and a single transport together, later transport changes or a Rust TC will force a rewrite of TM/RM business logic.

## Decision

Keep these layers separate:

1. `seata-protocol`: message types, request/response models, payload codecs, versioning and Java wire-code mappings.
2. `seata-transport`: connection lifecycle, framing, request correlation, heartbeat, reconnect and transport-specific adapters.
3. `seata-tm` / `seata-rm`: transaction semantics independent of transport.

V1 acceptance target is the native Seata V1 protocol used by Seata-Go, subject to live interoperability proof with the pinned Java Seata 2.x server.

gRPC is an optional transport adapter. RSeata's existing custom Protobuf service may be retained for internal experiments or a future Rust-native API, but it is not the Java compatibility protocol.

## Alternatives considered

- **Tonic-only client:** rejected because the RSeata service contract is custom and not equivalent to Java native remoting.
- **Hard-code Getty behavior into TM/RM:** rejected because Getty is a Go transport implementation, not the transaction protocol itself.
- **Implement all Java protocol versions in V1:** deferred; V1 first proves V1 native interoperability. V0/V2 are separately versioned compatibility increments.

## Consequences

- Payload codecs and transport adapters can be tested independently.
- Java 2.x V1 acceptance must be confirmed; the presence of a V1 decoder is not sufficient proof.
- The protocol layer must preserve sparse message IDs and exact field order.
- Serializer, compressor and header-map compatibility are first-class requirements.
- A later Rust TC can reuse the same message model and transport implementation.

## Validation required

- Java/Go/Rust golden byte vectors.
- Java 2.x live test using V1.
- Frame fragmentation, coalescing, heartbeat and request correlation tests.
- Serializer/compressor negotiation tests.

## Source evidence

- [Java ProtocolConstants](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/protocol/ProtocolConstants.java)
- [Java V1 encoder](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolEncoderV1.java)
- [Java V1 decoder](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolDecoderV1.java)
- [Go protocol selector](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/protocol.go)
- [RSeata custom proto](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-proto/proto/rseata.proto)