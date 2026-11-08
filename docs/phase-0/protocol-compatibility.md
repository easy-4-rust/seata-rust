# Phase 0 — Protocol Compatibility Specification

> **Status: Draft specification · compatibility target not yet proven**
>
> V1 target: Rust TM/RM interoperating with the pinned Apache Seata Java 2.x TC using the Seata native remoting protocol implemented by Seata-Go. gRPC is a separate optional transport path and is not the native Seata TCP frame.

## 1. Compatibility layers

The word "protocol" is overloaded. Seata-Rust must model these layers independently:

| Layer | Responsibility | V1 requirement |
|---|---|---|
| Transport | TCP connection, gRPC channel, reconnect, heartbeat, framing | Native Seata TCP first; gRPC optional |
| Frame envelope | Magic, version, frame length, header length, message type, codec, compressor, request ID | Byte-level compatible |
| Message model | Global/branch request and response fields | Field-level compatible |
| Payload codec | Java/Seata serializer and per-message payload encoding | Compatible serializer and exact field order |
| Transaction semantics | XID, BranchType, status codes, resource ID, lock keys, application data, retries | Semantic compatibility |
| Context propagation | XID propagation between services | Compatible XID and propagation contract |

A successful TCP connection is not proof of protocol compatibility. A decodable frame is not proof of message-payload compatibility. A matching enum name is not proof of numeric-code compatibility.

## 2. Native Seata frame: V1 facts

The Java V1 encoder/decoder and Go protocol constants establish the following V1 envelope:

| Field | Size | Meaning |
|---|---:|---|
| Magic | 2 bytes | `0xDA 0xDA` |
| Protocol version | 1 byte | V1 is `1` |
| Full length | 4 bytes | Total frame bytes, including header and body |
| Header length | 2 bytes | Header bytes including fixed header and optional header map |
| Message type | 1 byte | Request/response/heartbeat type |
| Serializer/codec | 1 byte | Payload serializer identifier |
| Compressor | 1 byte | Payload compressor identifier |
| Request ID | 4 bytes | Request/response correlation |
| Optional header map | variable | Key/value metadata |
| Body | variable | Serialized and optionally compressed message payload |

The fixed V1 header is 16 bytes. Java defines a maximum frame length of 8 MiB. Heartbeat request/response frames do not carry a normal serialized body.

### 2.1 Java source behavior

The pinned Java V1 encoder writes:

1. Magic bytes.
2. Protocol version.
3. A placeholder for full length and header length.
4. Message type, codec, compressor and request ID.
5. Optional header map.
6. Serialized payload, then compression.
7. Full/header lengths are backfilled.

The decoder validates magic, reads the same header fields, decodes optional header-map bytes, decompresses the body, then deserializes it.

Primary sources:
- [ProtocolConstants.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/protocol/ProtocolConstants.java)
- [ProtocolEncoderV1.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolEncoderV1.java)
- [ProtocolDecoderV1.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolDecoderV1.java)
- [ProtocolRpcMessageV1.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolRpcMessageV1.java)

### 2.2 Version negotiation risk

Java Seata 2.x declares protocol versions 0, 1 and 2; its current default `ProtocolConstants.VERSION` is 2. The V1 encoder explicitly emits version 1. Seata-Go's native protocol constants declare `VERSION = 1`, `V1HeadLength = 16`, and the same magic code.

**V1 decision:** implement the Seata-Go V1 frame first, but do not assume every Java 2.x server configuration accepts it. Verify the selected server's multi-protocol detector and V1 decoder with a live compatibility test. Protocol V2 support is a separate later decision.

## 3. Message type IDs

The Java `MessageType` and Go `message/constant.go` assign matching IDs for the core TM/RM operations.

| Message | ID | Direction |
|---|---:|---|
| GlobalBegin | 1 | TM → TC |
| GlobalBeginResult | 2 | TC → TM |
| BranchCommit | 3 | TC → RM |
| BranchCommitResult | 4 | RM → TC |
| BranchRollback | 5 | TC → RM |
| BranchRollbackResult | 6 | RM → TC |
| GlobalCommit | 7 | TM → TC |
| GlobalCommitResult | 8 | TC → TM |
| GlobalRollback | 9 | TM → TC |
| GlobalRollbackResult | 10 | TC → TM |
| BranchRegister | 11 | RM → TC |
| BranchRegisterResult | 12 | TC → RM |
| BranchStatusReport | 13 | RM → TC |
| BranchStatusReportResult | 14 | TC → RM |
| GlobalStatus | 15 | TM → TC |
| GlobalStatusResult | 16 | TC → TM |
| GlobalReport | 17 | TM → TC |
| GlobalReportResult | 18 | TC → TM |
| GlobalLockQuery | 21 | RM → TC |
| GlobalLockQueryResult | 22 | TC → RM |

Additional message families exist for registration, heartbeat, batch results and Undo Log cleanup. V1 implementation must not silently renumber or compact the sparse message IDs.

Sources:
- [Java MessageType.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/protocol/MessageType.java)
- [Go message constants](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/message/constant.go)

## 4. Request payload field mapping

The Go codecs are the most direct executable reference for Seata V1 payload field order. Java request classes establish the corresponding semantic fields. The table below records the observed Go codec field sequence; Java serializer interoperability must still be proven with golden vectors.

| Message | Observed payload field order |
|---|---|
| GlobalBeginRequest | timeout milliseconds as uint32; transaction name as length-prefixed string16 |
| GlobalCommitRequest | common global-end payload: XID string16; extra data string32 |
| GlobalRollbackRequest | common global-end payload: XID string16; extra data string32 |
| GlobalStatusRequest | common global-end payload |
| GlobalReportRequest | common global-end payload plus global status (verify exact codec) |
| BranchRegisterRequest | XID string16; BranchType byte; resource ID string16; lock key string32; application data string32 |
| BranchReportRequest | XID string16; Branch ID uint64; BranchStatus byte; resource ID string16; application data string32; BranchType byte |
| BranchCommitRequest | XID string16; Branch ID uint64; BranchType byte; resource ID string16; application data string32 |
| BranchRollbackRequest | XID string16; Branch ID uint64; BranchType byte; resource ID string16; application data string32 |

The exact byte order, signedness, string encoding, null/empty behavior, and serializer/compressor identifiers must be captured in Rust golden-vector tests. Do not infer these from the table alone.

### 4.1 Registration identity

TM/RM registration messages carry application identity and resource identity. The V1 implementation must map:

- application ID
- transaction service group
- resource group ID
- resource ID(s)
- client ID
- BranchType

to the exact upstream payload and registration lifecycle. Reconnect and resource re-registration are part of correctness, not an optional optimization.

## 5. XID contract

Java Seata's `XID.generateXID(transactionId)` produces:

```text
<server-ip>:<server-port>:<transaction-id>
```

The XID is generated by TC and returned in GlobalBeginResponse. TM binds it to the transaction context and propagates it to downstream services.

Seata-Go consumes the XID returned by TC. RSeata's inspected `DefaultGlobalSession::new` instead constructs `hex(transaction_id):transaction_service_group`, which is not Java XID format.

**V1 rule:** Rust TM must not generate a replacement XID. It must treat the TC-returned XID as opaque data, validate only the contractually required constraints, and propagate it unchanged.

Source: [Java XID.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/common/src/main/java/org/apache/seata/common/XID.java)

## 6. Transport modes

Seata-Go defines two protocol selections:

- `seata`: native Seata remoting framing and message codec.
- `grpc`: gRPC transport integration.

These are not interchangeable wire encodings. Seata-Rust should define a transport abstraction, but the V1 acceptance target is native Seata protocol compatibility.

Suggested boundary:

```text
seata-tm / seata-rm
        |
seata-protocol (message models + payload codecs)
        |
seata-transport (request correlation, connection, heartbeat)
        |
  +-----+------+
  |            |
Seata TCP    gRPC (optional)
```

## 7. RSeata custom gRPC contract is not the Java native protocol

RSeata's `rseata.proto` defines custom `TransactionManagerService`, `ResourceManagerService` and health services. Its messages use Protobuf field numbers and bidirectional resource instruction streams. That is a valid independent API design, but it is not the Java Seata V1 TCP frame or the Java transaction message payload contract.

RSeata Tonic can be retained as an optional internal API or future Rust-native server API. It cannot be used as evidence that a Rust client can connect to a stock Java Seata TC.

Source: [RSeata rseata.proto](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-proto/proto/rseata.proto)

## 8. Confirmed RSeata numeric mapping hazards

The inspected RSeata source contains incompatible numeric domains:

| Type | Java / Go | RSeata internal | RSeata proto | Risk |
|---|---|---|---|---|
| BranchType | AT=0, TCC=1, SAGA=2, XA=3 (Java also has SAGA_ANNOTATION) | AT=1, TCC=2, SAGA=3, XA=4 | AT=0, TCC=1, SAGA=2, XA=3 | Internal `From<i32>` maps 0 to AT and 1 to AT; direct casts shift meanings |
| GlobalStatus | Java/Go 0..20; AsyncCommitting=8, Committed=9 | 0..20, same ordering | omits AsyncCommitting and assigns Committed=8 | Status values 8 onward are shifted / not representable |
| BranchStatus | Java codes 0..15; Go 0..10 for its exposed branch model | Registered=1..Unknown=12 | Normal=0 and a different ordering 1..9 | Direct enum casts are unsafe |

The protocol layer must use explicit mapping functions with exhaustive match statements and reject unknown or unsupported values. Never serialize Rust enum discriminants as protocol codes.

## 9. Protocol acceptance tests

Before declaring V1 protocol-compatible, require:

1. Java-generated golden frame → Rust decoder.
2. Rust-generated golden frame → Java decoder.
3. Go-generated golden frame → Rust decoder.
4. Rust-generated golden frame → Go decoder.
5. Golden payload vectors for every V1 request/response message used in V1.
6. Header-map round trip.
7. Serializer and compressor negotiation.
8. Heartbeat request/response.
9. Fragmented TCP frames and multiple frames in one read.
10. Invalid magic, unsupported version, truncated frame, oversized frame, invalid header length, unknown message type and malformed payload rejection.
11. Request ID correlation, timeout, reconnect and duplicate response behavior.
12. A live Rust TM/RM ↔ Java TC integration suite on the pinned versions.

No byte vector should be handwritten from assumptions. Generate it from the pinned Java and Go implementations and store the provenance alongside the fixture.

## 10. Open protocol questions

- Which Java 2.x server configurations negotiate/accept V1 by default?
- Which serializer ID and serializer implementation does the selected Java server use by default?
- Are the Seata-Go `grpc` message payloads identical to native Seata payloads for all V1-required messages?
- Which optional header-map keys are required for registration, auth, application identity and cluster routing?
- What are the exact response body fields and retry/error-code semantics for each request?
- Which V2 changes are required for later Java 2.x compatibility?

These remain explicit Phase 0 research tasks; do not close them by assumption.