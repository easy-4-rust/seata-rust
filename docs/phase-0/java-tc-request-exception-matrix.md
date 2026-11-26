# Java Seata 2.x — V1 Request Exception / Result Mapping Matrix

> **Status (2026-11-12):** all selected handler/processor paths and their actual serialized outputs are now compared: Java135/135; capture78 responses+1unencodable diagnostic; Rust83/83. Go78 native checks split62 compatible/6 length differences/1 ordinal difference/9 opaque-state cases. Live auth and known issues remain open. [Original handler evidence](evidence/local-java-handler-matrix-20261112/README.md); [runtime comparison](evidence/local-handler-runtime-20261112/README.md).
>
> Java baseline: `apache/incubator-seata@cba73e180fd8896a99df6415b859d275122feaa7`.
>
> Main sources: `server/.../AbstractTCInboundHandler.java`, `core/.../AbstractExceptionHandler.java`, `server/.../DefaultCoordinator.java`, `core/.../processor/server/RegTmProcessor.java`, `RegRmProcessor.java`, and `ServerOnRequestProcessor.java`.

## 1. Common exception template

For transaction request/response types handled by `AbstractTCInboundHandler`, Java uses `AbstractExceptionHandler.exceptionHandleTemplate`:

| Execution outcome | Response mutation |
|---|---|
| Callback returns normally | `ResultCode.Success` |
| `TransactionException` | `ResultCode.Failed`; `TransactionExceptionCode = tex.getCode()`; `Msg = "TransactionException[" + tex.getMessage() + "]"` |
| Other `RuntimeException` | `ResultCode.Failed`; `Msg = "RuntimeException[" + rex.getMessage() + "]"`; transaction exception code is not set by this branch |
| Checked exception not declared as TransactionException | Not directly caught by this template; callback contract declares TransactionException and normal Java implementations must not leak checked exceptions |

Special log handling exists for `LockKeyConflict` and `LockKeyConflictFailFast`; it does not change the generic response mapping above.

A `StoreException` is explicitly wrapped as `TransactionException(FailedStore, ...)` in several handlers before reaching the common template.

Source:
- [AbstractExceptionHandler](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/exception/AbstractExceptionHandler.java)
- [AbstractTCInboundHandler](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/AbstractTCInboundHandler.java)

## 2. Per-request matrix

| Request | Handler entry | Initial response fields | StoreException mapping | Exception status fallback | Rust client interpretation |
|---|---|---|---|---|---|
| GlobalBegin | `handle(GlobalBeginRequest)` → `doGlobalBegin` | no initial XID; default result fields | wrapped as `FailedStore` | none | Failed result is definite rejection only when a response was decoded; timeout after send is unknown |
| GlobalCommit | `handle(GlobalCommitRequest)` → `doGlobalCommit` | GlobalStatus initialized to `Committing` | wrapped as `FailedStore` | on TransactionException and RuntimeException: lookup XID; use current session status, or `Finished` if absent | Preserve ResultCode, TransactionExceptionCode, Msg and GlobalStatus together |
| GlobalRollback | `handle(GlobalRollbackRequest)` → `doGlobalRollback` | GlobalStatus initialized to `Rollbacking` | wrapped as `FailedStore` | same as GlobalCommit | Preserve response status even when ResultCode is Failed |
| GlobalStatus | `handle(GlobalStatusRequest)` → `doGlobalStatus` | GlobalStatus initialized to `UnKnown` | wrapped as `FailedStore` | on TransactionException and RuntimeException: current session status or `Finished` | `Finished` means no session found in fallback; it is not equivalent to committed |
| GlobalReport | `handle(GlobalReportRequest)` → `doGlobalReport` | GlobalStatus initialized from request.GlobalStatus | no local StoreException wrapper in this callback | no custom status fallback; generic failure leaves the request status in response | Do not treat echoed status as proof that report mutation succeeded if ResultCode failed |
| BranchRegister | `handle(BranchRegisterRequest)` → `doBranchRegister` | no branch ID prefilled | wrapped as `FailedStore` | none | On failure preserve code/message; on timeout after send branch/lock outcome is unknown |
| BranchReport | `handle(BranchReportRequest)` → `doBranchReport` | standard transaction response | wrapped as `FailedStore` | none | Do not discard failed response; retry/report reconciliation must account for possible already-persisted status |
| GlobalLockQuery | `handle(GlobalLockQueryRequest)` → `doLockCheck` | standard transaction response; Lockable default false | wrapped as `FailedStore` | none | `Lockable=false` is a valid lock conflict result only with a decoded successful response; transport error is not false |
| RegisterTM | remoting registration listener, not `AbstractTCInboundHandler` | `RegisterTMResponse(identified)` | auth false/throw yields identified=false and Java-object message; V1 omits that message | none | Treat wire identified=false as rejection; Java decoded ResultCode can remain Success because that field is not transmitted |
| RegisterRM | remoting registration listener, not `AbstractTCInboundHandler` | `RegisterRMResponse(identified)` | same separate IdentifyResponse control plane | none | Same; registration is channel/resource association, repeat after reconnect |
| BranchCommit / BranchRollback | TC → RM phase-2 instruction, not a TM/RM request handled by TC inbound `AbstractTCInboundHandler` | RM returns branch status/result | not applicable to TC inbound request matrix | TC consumes RM response and decides branch removal/retry/global outcome | Rust RM must return Java-compatible BranchStatus and be idempotent under duplicate instruction |

### 2.1 Global status fallback is intentionally narrow

`checkTransactionStatus` is called only by GlobalCommit, GlobalRollback and GlobalStatus callback exception handlers.

It performs:

- `SessionHolder.findGlobalSession(xid, false)`;
- if present, sets response.GlobalStatus to the session's current status;
- if absent, sets response.GlobalStatus to `Finished`;
- if the lookup or session status getter throws, logs the exception and leaves the previous response status unchanged. If the core callback already changed it before failing, that changed value is retained; this is not always the initial Committing/Rollbacking/UnKnown value.

It is not used by GlobalBegin, GlobalReport, BranchRegister, BranchReport or GlobalLockQuery.

This means the Rust decoder must not invent a universal “exception status fallback” rule.

## 3. StoreException and transaction error code details

The following StoreException wrappers are explicit in `AbstractTCInboundHandler`:

| Request | TransactionExceptionCode | Message context |
|---|---|---|
| GlobalBegin | FailedStore | begin global request failed; includes response XID (which may not yet exist) |
| GlobalCommit | FailedStore | global commit request failed; XID |
| GlobalRollback | FailedStore | global rollback request failed; XID |
| GlobalStatus | FailedStore | global status request failed; XID |
| BranchRegister | FailedStore | branch register request failed; XID |
| BranchReport | FailedStore | branch report request failed; XID + BranchId |
| GlobalLockQuery | FailedStore | global lock query request failed; XID |
| GlobalReport | no explicit wrapper in this handler | direct `doGlobalReport` call |

Other transaction error codes originate in DefaultCore / branch-type Core and must be preserved as Java enum ordinal values. The Java `TransactionExceptionCode` enum is ordinal-coded; Rust must use explicit mapping rather than Rust enum discriminants.

## 4. Registration is a separate control plane

The Java remoting listener handles RegisterTM and RegisterRM separately from transaction request dispatch:

1. Decode the registration request.
2. Run the corresponding optional auth hook.
3. Register the channel with `ChannelManager.registerTMChannel` or `registerRMChannel`.
4. Store client protocol version on the channel.
5. Catch registration exceptions and return an identified=false response with optional message.

Here "message" is a Java-object field only: native V1 IdentifyResponse writes identified+version. Runtime tests prove the error text is lost on this wire encoding. Deserialization constructs the default-success response object and changes identified without reading ResultCode, so **identified=false with Java ResultCode.Success** can occur after decode. Clients must inspect identified, not the inherited ResultCode.

The registration response's `Identified` flag is not the same field as the transaction `ResultCode`. Rust should model these as separate response types.

Sources:
- [RegTmProcessor](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/processor/server/RegTmProcessor.java)
- [RegRmProcessor](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/processor/server/RegRmProcessor.java)
- [ServerOnRequestProcessor](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/processor/server/ServerOnRequestProcessor.java)

## 5. Request-level failure categories for Rust

| Rust category | Evidence | Retry rule |
|---|---|---|
| Definite TC rejection | decoded response with ResultCode.Failed or Identified=false | do not retry blindly; apply request-specific business policy |
| Definite TC result | decoded success response | still inspect GlobalStatus / BranchStatus / Lockable / Identified |
| Unknown outcome | request may have been written but response timed out/lost | reconcile by XID/BranchId/status; never convert to definite rejection |
| Local pre-send failure | connection unavailable or serialization fails before write | safe to retry after fixing transient local/transport condition |
| Protocol failure | invalid frame, invalid type, malformed payload, unknown ordinal | fail closed; do not retry identical bytes |
| TC internal/store failure | decoded FailedStore response | surface typed error; status fallback only for Commit/Rollback/Status |

## 5.1 Runtime verification result

The overlay suite `tools/java-v1-golden/AbstractTCInboundHandlerExceptionTest.java` passed against the pinned Java source in [GitHub Actions Run #18](https://github.com/easy-4-rust/seata-rust/actions/runs/42458368199). It adds eight test methods covering:

- GlobalBegin TransactionException, RuntimeException and StoreException wrapping;
- GlobalCommit exception status fallback when a session exists and when it is absent;
- GlobalStatus exception status fallback when a session exists;
- GlobalReport RuntimeException preserving the request status without status lookup;
- BranchRegister StoreException wrapping as FailedStore.

Combined with the four V1 frame tests, Maven reports **12 tests, 0 failures, 0 errors, 0 skipped**. This verifies the selected handler cases, not every row in the full request matrix.

## 6. Verification tasks

- [x] Overlay JUnit tests for selected request TransactionException, RuntimeException and StoreException behavior (GlobalBegin, GlobalCommit, GlobalStatus, GlobalReport, BranchRegister); see runtime result above.
- [x] Extend runtime tests to GlobalRollback, BranchReport and GlobalLockQuery — all eight handlers × four outcomes =32 cases.
- [x] Verify Commit/Rollback/Status fallback with present/absent session — all three failure kinds.
- [x] Verify lookup/getter throws retain the previous status, including callback-modified status —36 additional fallback cases cover four edge scenarios × three handlers × three failure kinds.
- [x] Verify BranchRegister both LockKeyConflict and LockKeyConflictFailFast return failed result/code, not a fabricated BranchId.
- [x] Verify actual RegTM/RM auth-hook allow/false/throw and V1 Identified semantics independently of inherited ResultCode —7 processor unit cases including closed denied RM cleanup. This is not live credential authentication.
- [x] Verify malformed/empty payload rejection for the independent Rust probe's supported samples — every-prefix/trailing/enum/length tests, original29 and supplemental failure evidence. This is bounded negative testing, not arbitrary-fuzz proof or formal production decoder certification.
- [x] Compare all78 observed serialized handler/registration outputs against Go and Rust after a fresh Java run. Require all79 named contexts, recording the null-report encoding failure without fabricated bytes; classify Go differences/opaque states rather than calling them compatible. See runtime evidence above.

## 7. Runtime matrix increment and open boundary

`TCInboundFailureMatrixTest` adds72 actual-handler tests:32 main outcomes +36 fallback edge cases +2 conflict codes +successful lockable=false +null-report diagnostic. It asserts independent error text/code, status/identity/lock fields and manual full V1 bytes for every serializable ordinary outcome. The missing three injection paths in the old-style test fixture were first deliberately left inactive; new assertions failed as expected, then the fixture was completed. This RED proves the guard detects absent fault injection; it is not a Java production bug fix.

`RegistrationProcessorAuthTest` adds7 actual-processor cases, verifying auth invocation, no channel-registration calls on rejection, allowed calls, RM closed-channel cleanup and actual encoded/decoded flag/version semantics. Mocks isolate SPI loading/channel bookkeeping; they do not establish a live authenticated server or real global channel-map behavior.

**JAVA-V1-REPORT-NULL-001 remains OPEN:** a GlobalReport request can carry null status (legal request sentinel `0xff`), and a failed handler response retains null. The pinned GlobalReportResponse codec dereferences that status and throws NullPointerException while encoding. The diagnostic asserts Failed/null and the actual exception; it does not claim a usable wire error response. Future API guards/error handling need an explicit verified policy, not silent conversion to Unknown or successful report.

Combined selected Java target135/135 passed, zero failures/errors/skips;91-module BUILD SUCCESS. The earlier8 exception cases remain and are not replaced. A later fresh135 run now captures every serializable output and compares all78 against Go/Rust, requiring full case identities rather than relying on previous fixed examples. Rust fields and bytes match; Go diagnostic preserves explicit differences and opaque-state limits. The selected matrix's tests/comparison are complete, while Phase0/WP00, known issues and the complete delivery goal stay open.
