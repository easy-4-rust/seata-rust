# Java Seata 2.x — V1 Request Exception / Result Mapping Matrix

> **Status:** pinned-source-derived. No runtime test is claimed.
>
> Java baseline: `apache/incubator-seata@cba73e180fd8896a99df6415b859d275122feaa7`.
>
> Main sources: `server/.../AbstractTCInboundHandler.java`, `core/.../AbstractExceptionHandler.java`, `server/.../DefaultCoordinator.java`, `core/.../DefaultServerMessageListenerImpl.java`, and `core/.../processor/server/ServerOnRequestProcessor.java`.

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
| RegisterTM | remoting registration listener, not `AbstractTCInboundHandler` | `RegisterTMResponse(identified)` | listener catches Exception from auth/channel registration and returns identified=false plus message | none | Treat identified=false as registration rejection; connection loss means registration unknown |
| RegisterRM | remoting registration listener, not `AbstractTCInboundHandler` | `RegisterRMResponse(identified)` | listener catches Exception from auth/channel registration and returns identified=false plus message | none | Same; registration is channel/resource association, repeat after reconnect |
| BranchCommit / BranchRollback | TC → RM phase-2 instruction, not a TM/RM request handled by TC inbound `AbstractTCInboundHandler` | RM returns branch status/result | not applicable to TC inbound request matrix | TC consumes RM response and decides branch removal/retry/global outcome | Rust RM must return Java-compatible BranchStatus and be idempotent under duplicate instruction |

### 2.1 Global status fallback is intentionally narrow

`checkTransactionStatus` is called only by GlobalCommit, GlobalRollback and GlobalStatus callback exception handlers.

It performs:

- `SessionHolder.findGlobalSession(xid, false)`;
- if present, sets response.GlobalStatus to the session's current status;
- if absent, sets response.GlobalStatus to `Finished`;
- if the lookup itself throws, logs the exception and leaves the previous response status unchanged.

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

The registration response's `Identified` flag is not the same field as the transaction `ResultCode`. Rust should model these as separate response types.

Sources:
- [DefaultServerMessageListenerImpl](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/rpc/DefaultServerMessageListenerImpl.java)
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

## 6. Verification tasks

- [ ] Overlay JUnit tests for each request's TransactionException and RuntimeException behavior.
- [ ] Verify Commit/Rollback/Status status fallback when session exists and when it is absent.
- [ ] Verify status fallback lookup itself throws and leaves initial status unchanged.
- [ ] Verify BranchRegister lock conflict maps LockKeyConflict and returns failed result.
- [ ] Verify RegisterTM/RM auth rejection produces Identified=false independently of ResultCode.
- [ ] Verify malformed/empty response payload cannot be interpreted as success.
- [ ] Compare all observed response bytes against Go and Rust codecs after the runtime harness executes.

No test execution is claimed by this source audit.
