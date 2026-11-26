# V1 TM/RM Registration Wire Contract

## Formal Rust session-readiness implementation contract (2026-11-13)

Continue WP-02's every-new-authenticated-session registration task using this existing contract and ADR-0008. This is not a production credential policy or completion of RM business callbacks.

- A registered pool selects exactly one native role per physical session, TM or RM. Java `ChannelManager.registerTMChannel` builds a TM role context and `registerRMChannel` builds/reuses an RM context; a convenience combined-role registration on one channel is not inferred. Raw pool startup remains explicit and does not acquire a business-ready label.
- Identity version/application/group and optional extraData are caller supplied and bounded by native string16. Debug/log/observability must not print them. Existing JAVA-AUTH-EXTRADATA-001 remains open: TC may log extraData, so only public test credentials are used in live fixtures; a successful identified flag proves the configured TC hook accepted the registration, not a production authentication mechanism.
- A bounded in-process resource registry retains exact `(BranchType, resource_id)` keys, monotonically checked revision and insertion order. Duplicate insertion is idempotent and does not change revision. Empty/comma-containing IDs are rejected: pinned Java `Constants.DBKEYS_SPLIT_CHAR=","` and `ChannelManager.dbKeytoSet` split that field without an escaping scheme. The payload codec still supports Java's full resource-list strings; this stricter registry boundary prevents one logical ID silently turning into multiple registrations. Resource removal and durable RM completion records remain later RM lifecycle work.
- Every new RM session registers each required cached resource individually using type103/104 and checks `identified`; the first resource-bearing request establishes its identity as well. This supports TC hooks that require a resource in the initial authentication request. Only an empty resource registry sends RM registration with no resource list, which the configured TC hook may legitimately reject. The report's identity and first-resource views refer to that same attempt, not two wire calls. A TM session performs type101/102 only. Registration attempts share one absolute initialization deadline; per-resource iteration cannot multiply the configured timeout. Wrong response families remain protocol failures; identified=false is explicit rejection, while lost response preserves the transport call's local/unknown classification.
- A bounded report records identity and every required resource as not attempted, accepted, rejected, local failure or unknown outcome, with typed transport errors where applicable. On the first failed prerequisite, later items remain not attempted and the session is closed; the next generation retries initialization from the whole current registry, never assuming a prior generation's partial registration succeeded. Initialization retry is identity/resource binding, not transaction/BranchRegister replay.
- Registered slots expose Registering, Ready and Degraded in addition to raw lifecycle states. Only a completely accepted report for the exact current registry revision can be selected by a registered pool. Registry additions invalidate readiness immediately in selection/snapshot checks; the supervisor reinitializes the same live session before publishing its updated revision. Requests already in flight retain their original response/error bookkeeping. Low-level acquired ConnectionHandle remains a generation-bound transport handle, not an eternal ready capability; TM/RM entry points must use the registered pool and registered resource identity.
- Shutdown/Drop must also terminate sessions currently registering, cancel their waiting futures and stop retries. Resource-revision changes cancel outdated initialization, retain bounded observability and rerun the current plan; late control replies cannot complete a newer request because IDs are not reused. Inbound callbacks received during initialization stay in the existing bounded session queue; they are forwarded only after readiness, with ordinary overflow/closure behavior.
- Required evidence: withheld identity/resource replies prevent business sends; explicit identity denial and partial resource denial stay not-ready with per-resource outcomes; new generations repeat all required registrations; registry additions invalidate a previously ready snapshot and register the new revision; unknown registration response stays distinct from rejection; role/version/nullable fields, typed resource keys, delimiter rejection, budget and cancellation/close behavior have tests. Live pinned Java TC verification remains separate from loopback protocol fault injection.

This work does not waive NET-010–012 transaction reconciliation, TLS/production authentication, the external TC findings or the deferred shared-lock issue.

Implementation verification: [formal session-registration Gate](../phase-1/evidence/session-registration/README.md), 76tests onRust1.99/1.85 and six actual pinned JavaTC cases. The first-resource RM compatibility control has an actual RED/GREEN; latest Java execution freezes actual runtime SPI files and proves five loaded-class origins. This closes only the WP-02 per-session registration implementation task; full WP02/V1 and business callback/recovery gates remain open.

> **Status (2026-11-12):** pinned codec/frame, live registration and bounded auth-hook/reconnect evidence are available. The checker is a local fixture; production authentication and formal Rust TM/RM remain undelivered.

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

The fixed processors' auth-hook false/throw paths now have unit evidence: the original Java response has ResultCode.Failed and an error message, but native V1 omits both fields. A newly decoded Java response can therefore have inherited ResultCode.Success while `identified=false`; error text is unset. This confirms why clients must use the identified flag. Processor allow/deny/throw tests and RM closed-channel cleanup are [recorded here](evidence/local-java-handler-matrix-20261112/README.md); REG-003/004 live authentication is not claimed from them.

A zero-length response version leaves Java's constructor default `Version.getCurrent()` intact (`2.8.0-SNAPSHOT` in the pinned build), rather than setting null/empty. Its Java re-encoding emits that default explicitly. Rust test codecs preserve original wire absence in `wire_version` and obtain the Java semantic value through `java_version(java_current)`; neither the TM runtime client version nor a guessed server version is substituted. [Three Java boundary tests and Rust evidence](evidence/local-rust-payloads-20261112/README.md) verify this independently of the 29 normal samples.

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

### Zero-length ExtraData differs by request codec

Pinned Java `AbstractIdentifyRequestCodec.decode`, used by RegisterTM, still calls `setExtraData(new String(new byte[0], UTF8))` at zero length, yielding `""`. RegisterRM overrides decoding and only invokes the setter when length is positive, leaving `null` at zero length. Both encode the same zero-length bytes. The all-payload Java corpus test verifies these two distinct object semantics; ADR-0008's optionality rule must therefore be applied per field/codec, not by converting every zero-length string to `None`. [Runtime evidence](evidence/local-java-payloads-20261112/README.md).

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

REG-001 and REG-002 have live Java TC evidence in Run #23 for the auth-disabled test configuration. REG-003/004 now pass locally for a separate test SPI:16 actual denials, identified=false, same-channel GlobalBegin closure and zero FILE allocation. REG-005/006 pass for one fresh authenticated RM connection re-registering two cached resources and handling two original-identity branch callbacks without branch re-registration. [Evidence](evidence/local-auth-registration/README.md). The registration frame/payload sub-gate is independently verified in Run #42. These scoped results do not deliver a production credential policy or durable RM reconnect.

The original default auth provider is permissive even with checks enabled; disabling checks bypasses the test extension. Original INFO registration logs also include extraData, so production credentials cannot assume that field is log-safe. [Runtime boundaries and open logging issue](auth-runtime-boundaries.md).
