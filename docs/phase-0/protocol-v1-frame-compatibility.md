# Java Seata V1 Frame Compatibility — Source Audit & Executable Fixture Plan

> **Status:** Java V1 decoder/encoder support verified in pinned source; cross-runtime integration not executed yet.
>
> Java source: `apache/incubator-seata@cba73e180fd8896a99df6415b859d275122feaa7`.
> Go source: `apache/incubator-seata-go@82570d905919c1c66bb025a3a5979f25390de498`.

## 1. Finding: Java Seata 2.x contains a V1 decoder, encoder and serializer

The pinned Java snapshot has:

- `core/.../ProtocolConstants.java`: `VERSION_0=0`, `VERSION_1=1`, `VERSION_2=2`; default `VERSION=VERSION_2`; V1 fixed header length 16.
- `core/.../netty/v1/ProtocolDecoderV1.java`: reads the V1 header, head map and body; exposes `protocolVersion() == VERSION_1`; loads the serializer for V1.
- `core/.../netty/v1/ProtocolEncoderV1.java`: writes the 16-byte V1 fixed header, optional head map and compressed serialized body; exposes `protocolVersion() == VERSION_1`.
- `serializer/.../SeataSerializer.java`: explicitly selects `SeataSerializerV1` when serializer version is 1.
- Java's `ProtocolDecoderV1Test` contains V1 heartbeat and head-map decoding tests.
- Java's `test-suite/test-new-version/.../ProtocolV1SerializerTest` contains a V1 client/server round-trip test.

**What this proves:** Java Seata 2.x source still implements V1 wire support. The default protocol version is V2, so the existence of V1 classes does not by itself prove that every deployment/configuration negotiates or selects V1. Runtime channel setup and server/client compatibility must still be exercised.

## 2. V1 fixed frame layout

The Java V1 decoder/encoder documents and implements this fixed 16-byte header:

| Offset | Size | Field | Encoding |
|---:|---:|---|---|
| 0 | 2 | Magic | `DA DA` |
| 2 | 1 | Protocol version | `01` |
| 3 | 4 | Full length | big-endian uint32; includes header + optional head map + body |
| 7 | 2 | Head length | big-endian uint16; includes fixed 16-byte header + head map |
| 9 | 1 | Message type | 0 sync request, 1 response, 2 one-way, 3 heartbeat request, 4 heartbeat response |
| 10 | 1 | Serializer | selected serializer code; Seata serializer is 1 in the pinned mapping |
| 11 | 1 | Compressor | NONE is 0 in the selected fixture |
| 12 | 4 | Request ID | big-endian uint32 |
| 16 | variable | Head map | repeated string16 key + string16 value; absent in baseline fixtures |
| headLength | variable | Body | compressed serialized message payload |

Length invariants:

```text
fullLength = 16 + headMapLength + compressedBodyLength
headLength = 16 + headMapLength
bodyLength = fullLength - headLength
```

The fixed header is not a 16-byte body prefix. TCP frame length includes all bytes from magic through body, while the V1 Netty length-field decoder reads the full length at offset 3 with a 4-byte field and `lengthAdjustment=-7`.

## 3. Body composition

For Seata serializer V1, Java's `SeataSerializerV1.serialize` writes:

```text
uint16_be(message_type_id) || message_specific_codec_body
```

The pinned Go `CodecManager.Encode` performs the same operation: it prepends the big-endian uint16 message type to the codec's body bytes.

Therefore three distinct levels must be compared:

1. **Codec body** — message-specific fields only; stored as `body_hex` in the 25-vector table.
2. **Serialized message payload** — two-byte type ID + codec body; stored as `serialized_hex` in `tools/go-golden-vectors/expected.json`.
3. **V1 TCP frame** — fixed header + optional head map + serialized payload; examples are in `tools/go-golden-vectors/frames.json`.

A test that compares only level 1 does not prove Java frame compatibility.

## 4. Committed deterministic frame examples

The following are source-derived candidate frames with no head map, request ID 1, Seata serializer 1 and NONE compressor 0.

| Frame | Request type | Full length | Expected frame hex |
|---|---:|---:|---|
| GlobalBegin request | 0 | 42 | `dada010000002a0010000100000000010001000007d000125365617461476f5472616e73616374696f6e` |
| GlobalBegin response | 1 | 40 | `dada01000000280010010100000000010002010000103132372e302e302e313a383039313a310000` |

The Go verification harness reconstructs these frames from the actual Go serializer payload bytes and checks exact equality against the committed frame fixtures.

## 5. Java decoder behavior and caveats

Source observations from `ProtocolDecoderV1.decodeFrame`:

- Validates magic bytes `DA DA`.
- Reads the protocol version byte, but the shown method does not reject a non-1 value at that point.
- Reads full length and head length from the frame.
- Decodes the optional head map when `headLength > 16`.
- Treats heartbeat request/response as bodyless control messages.
- For ordinary messages, decompresses the body, resolves the serializer from the codec ID, and deserializes using protocol version 1.
- The frame decoder enforces the configured 8 MiB maximum frame length through Netty's `LengthFieldBasedFrameDecoder`.

Source observations from `ProtocolEncoderV1.encode`:

- Starts fixed header length at 16.
- Writes optional head map entries as string16 key/value pairs.
- Serializes the body with the serializer selected for protocol version 1.
- Compresses the serialized bytes.
- Patches full length and head length after writing.

Important caveat: Java's `ProtocolConstants.VERSION` defaults to 2. The existence of a V1 codec is not a substitute for checking the actual protocol selection path and configuration used by the Java TC deployment.

## 6. Remaining runtime interoperability test

The next executable Java test should use the pinned Java checkout and test both directions:

### A. Rust/Go-produced frame → Java decoder

1. Construct the same typed GlobalBegin request and response objects.
2. Generate the complete V1 frame with serializer=SEATA and compressor=NONE.
3. Feed the raw frame bytes into Java `ProtocolDecoderV1.decodeFrame` (or the actual Netty pipeline).
4. Assert request type, request ID, serializer, compressor, message class and every decoded field.
5. Repeat for every V1 message in the 25-vector set.

### B. Java encoder → Rust/Go decoder

1. Construct Java message instances with identical fixture values.
2. Serialize using `SeataSerializerV1`.
3. Encode with `ProtocolEncoderV1`.
4. Capture raw frame bytes.
5. Compare exact serialized payload and frame bytes with the Go/Rust implementations.
6. Repeat with non-empty head maps, null header values, compressed bodies and failure responses.

### C. Negative frame tests

- Wrong magic.
- Unsupported/unknown protocol version.
- Full length smaller than fixed header.
- Head length smaller than 16 or greater than full length.
- Truncated head map key/value.
- Truncated serialized payload.
- Unknown serializer ID.
- Unknown compressor ID.
- Oversized frame.
- Unknown message type.
- Heartbeat request/response with unexpected body bytes.

## 6.1 Reproducible Java decoder/encoder test

A JUnit overlay test is now committed at `tools/java-v1-golden/ProtocolV1GoldenFrameTest.java`. It feeds both complete Go-derived frames to Java `ProtocolDecoderV1`, asserts decoded message class and fields, then passes the decoded RpcMessage through Java `ProtocolEncoderV1` and requires exact full-frame byte equality.

Run from the Seata-Rust repository root:

```bash
bash tools/java-v1-golden/verify.sh
```

The script checks out the pinned Java commit `cba73e180fd8896a99df6415b859d275122feaa7`, overlays the test into the existing `test-suite/test-new-version` module (which already depends on the SEATA serializer), and runs the targeted Maven test through the reactor.

**Execution status in this research session:** the test and script are committed but Maven has not been executed here. Java V1 source support is verified; this test remains pending runtime execution.

## 7. Evidence status

| Claim | Status |
|---|---|
| Java source has V1 constants and V1 encoder/decoder | **Verified in source** |
| Java V1 serializer prepends uint16 message type | **Verified in source** |
| Go V1 serializer payload prepends the same type ID | **Verified in source** |
| Go harness has 25 codec vectors and 2 frame fixtures | **Committed; not executed in this session** |
| Java harness overlays two complete frame round-trip tests on the pinned test-suite module | **Committed; not executed in this session** |
| Java accepts all 25 Go-produced V1 payloads | **Unknown; runtime test not executed** |
| Java and Go produce byte-identical complete frames | **Unknown; runtime test not executed** |
| Rust matches Java/Go V1 frame bytes | **Not implemented / not tested** |

## 8. Pinned source references

- [Java ProtocolConstants](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/protocol/ProtocolConstants.java)
- [Java ProtocolDecoderV1](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolDecoderV1.java)
- [Java ProtocolEncoderV1](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/netty/v1/ProtocolEncoderV1.java)
- [Java HeadMapSerializer](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/netty/v1/HeadMapSerializer.java)
- [Java SeataSerializer](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/serializer/seata-serializer-seata/src/main/java/org/apache/seata/serializer/seata/SeataSerializer.java)
- [Java V1 decoder tests](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/test/java/org/apache/seata/core/rpc/netty/v1/ProtocolDecoderV1Test.java)
- [Java V1 client/server serializer test](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/test-suite/test-new-version/src/test/java/org/apache/seata/core/rpc/netty/v1/ProtocolV1SerializerTest.java)
- [Go codec manager](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/codec/codec.go)
- [Go message IDs and V1 constants](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/message/constant.go)
