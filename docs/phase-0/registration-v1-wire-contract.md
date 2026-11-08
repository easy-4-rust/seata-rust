# V1 TM/RM Registration Wire Contract

> **Status:** source-derived contract with Go-generated payload fixtures and Java V1 frame round-trip tests added. Live Java TC registration remains unverified until the TC bootstrap and independent client execute against the running server.

## 1. Pinned source authority

- Java Seata: `apache/incubator-seata@cba73e180fd8896a99df6415b859d275122feaa7`
- Seata-Go: `apache/incubator-seata-go@82570d905919c1c66bb025a3a5979f25390de498`

Key Java files:
- `core/.../protocol/RegisterTMRequest.java`
- `core/.../protocol/RegisterTMResponse.java`
- `core/.../protocol/RegisterRMRequest.java`
- `core/.../protocol/RegisterRMResponse.java`
- `core/.../protocol/AbstractIdentifyRequest.java`
- `core/.../protocol/AbstractIdentifyResponse.java`
- `serializer/seata-serializer-seata/.../AbstractIdentifyRequestCodec.java`
- `serializer/seata-serializer-seata/.../AbstractIdentifyResponseCodec.java`
- `serializer/seata-serializer-seata/.../RegisterRMRequestCodec.java`
- `core/.../rpc/processor/server/RegTmProcessor.java`
- `core/.../rpc/processor/server/RegRmProcessor.java`
- `core/.../rpc/netty/MultiProtocolDecoder.java`

## 2. Message type IDs and response model

| Role | Java type | Message type ID | Response |
|---|---|---:|---|
| TM registration | RegisterTMRequest | 101 | RegisterTMResponse (102) |
| RM registration | RegisterRMRequest | 103 | RegisterRMResponse (104) |

Registration uses **IdentifyResponse**, not the ordinary transaction ResultCode codec. The V1 body is:

- response: `identified: u8` (0 or 1), followed by `version: string16`;
- success must be determined from `identified == 1`, not from parsing a transaction response status.

The Java response class also inherits result/message fields, but the Seata serializer's IdentifyResponse codec writes only the identified byte and version string. Do not invent additional ResultCode fields in this wire body.

## 3. Request body layout

All integer lengths are big-endian.

### RegisterTMRequest (type 101)

```text
version                  string16
applicationId            string16
transactionServiceGroup  string16
extraData                string16
```

The Java `RegisterTMRequest(applicationId, transactionServiceGroup, extraData)` constructor may append `vgroup=<group>` and local `ip=<ip>` entries to extraData. The Go codec serializes the fields supplied by its caller; the codec itself does not synthesize these entries. Treat this as a request-construction behavior, not a codec behavior.

### RegisterRMRequest (type 103)

```text
version                  string16
applicationId            string16
transactionServiceGroup  string16
extraData                string16
resourceIds              string32
```

The RM resource ID is a 32-bit-length-prefixed UTF-8 string. It is not another string16 field.

## 4. V1 frame behavior

The client sends registration as a synchronous V1 request frame:
- protocol version byte = 1;
- request message type byte = 0;
- serializer ID = Seata;
- compressor ID = NONE for the current fixtures;
- request ID is non-zero and correlated with the response.

The server's `MultiProtocolDecoder` reads the incoming frame's protocol version byte, selects the corresponding decoder/encoder pair (V0/V1/V2), and installs that pair into the channel pipeline. Although Java `ProtocolConstants.VERSION` defaults to 2, a V1 frame selects the V1 decoder/encoder for that channel. The default constant alone therefore does not prove V1 is rejected. The live server must still verify this behavior end-to-end.

## 5. Registration processing

### TM

```text
V1 frame (type 101)
  -> RegTmProcessor
  -> optional RegisterCheckAuthHandler
  -> ChannelManager.registerTMChannel(request, channel)
  -> RegisterTMResponse(identified=true|false)
  -> V1 response frame (type 102)
```

Java records the client version on the channel. Successful registration associates the TM channel with the supplied application ID and transaction service group.

### RM

```text
V1 frame (type 103)
  -> RegRmProcessor
  -> optional RegisterCheckAuthHandler
  -> ChannelManager.registerRMChannel(request, channel)
  -> RegisterRMResponse(identified=true|false)
  -> V1 response frame (type 104)
```

RM registration associates the resource IDs and transaction service group with the channel. Java also removes channel bookkeeping if the channel has already closed while registration was being processed.

A successful RM registration is a prerequisite for TC phase-two callbacks. It does not prove that the client can execute BranchCommit/BranchRollback.

## 6. Current deterministic fixtures

Go source harness constructs RegisterTM/RM payloads in `tools/go-golden-vectors/main.go`. Their serialized payloads are already part of `expected.json` and the 29-vector Go runtime test set.

Two full V1 registration frames have now been added to `frames.json` and passed the pinned Go and Java runtime harness in [GitHub Actions Run #42](https://github.com/easy-4-rust/seata-rust/actions/runs/42474150028):
- `RegisterTMRequestFrame`: request ID 2, full length 57 bytes.
- `RegisterRMRequestFrame`: request ID 3, full length 75 bytes.

Java V1 Decoder → Encoder byte-identical round-trip tests have been added to `ProtocolV1GoldenFrameTest.java`. Run #42 verified all 29 Go payload vectors and 6 complete frames; Java passed all 6 frame decode/re-encode tests. The full Java overlay run reported 17 tests with 0 failures/errors. This certifies the tested codec/frame bytes, not a live TC registration exchange.

## 7. Authentication boundary

The bootstrap FILE configuration disables TC auth checks only to make the server-start smoke deterministic. This is not a production security recommendation and does not prove authentication compatibility.

Before production-like integration:
1. run RegisterTM/RM with auth disabled and verify channel registration;
2. enable TC auth and test the actual Java auth handler contract;
3. test denied registration and confirm `identified=false` response semantics;
4. test reconnect and re-registration on a new TCP channel.

## 7.1 Live Java TC registration evidence

[GitHub Actions Run #23](https://github.com/easy-4-rust/seata-rust/actions/runs/42475353541) passed the independent Rust client's live V1 registration against the pinned Java TC:
- RegisterTM response type 102: `identified=true`.
- RegisterRM response type 104: `identified=true`, resource ID `mysql://orders`.
- Both response frames used protocol V1 and reported Java server version `2.8.0-SNAPSHOT`.

The run used the test-only configuration `enable-check-auth=false`. Authentication success/denial semantics are not covered. The initial live client sent version `V1,0` from the codec fixture; the current Rust client is being aligned to the actual pinned Go runtime constant `constant.SeataVersion = "1.1.0"`. The next CI run must reconfirm registration with that value.

## 8. Integration test gates

| ID | Test | Expected evidence |
|---|---|---|
| REG-001 | RegisterTM over live Java TC | type 102 response, identified=true |
| REG-002 | RegisterRM over live Java TC | type 104 response, identified=true |
| REG-003 | RegisterTM denied by auth | identified=false and channel not bound |
| REG-004 | RegisterRM denied by auth | identified=false and no resource association |
| REG-005 | RM reconnect | fresh channel re-registers all cached resource IDs |
| REG-006 | TC callback routing | TC finds the registered RM channel by resource ID/client association |

REG-001 and REG-002 have live Java TC evidence in Run #23 for the auth-disabled test configuration. REG-003/004 auth-denial and REG-005 reconnect remain open. The registration frame/payload sub-gate is verified independently in Run #42.
