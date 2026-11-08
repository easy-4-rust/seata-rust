# Seata-Go v2 — Transport, Timeout, Correlation and Reconnect Semantics

> **Status:** pinned-source-derived. This document does not claim Java/Go/Rust runtime interoperability.
>
> Go baseline: `apache/incubator-seata-go@82570d905919c1c66bb025a3a5979f25390de498`.

## 1. Two transport implementations, one request/response abstraction

Seata-Go has two remoting implementations selected by configuration:

| Concern | Getty / native Seata | gRPC |
|---|---|---|
| Main remoting | `pkg/remoting/getty/*` | `pkg/remoting/grpc/*` |
| Client request ID | atomic uint32 | atomic uint32 |
| Request correlation | `sync.Map` of MessageFuture by ID | same pattern |
| Message body | Seata native message codec | generated Protobuf message |
| Serializer/codec | CodecTypeSeata | CodecTypeGRPC |
| Wait timeout | 20 seconds | 20 seconds |
| Sync API | `SendSyncRequest` | `SendSyncRequest` |
| RM reconnect registration | session-open hook | channel-open hook |
| Java V1 native frame | intended native Seata path; verify against Java | not the same native Seata V1 payload; separate gRPC/Protobuf contract |

The two transports share high-level TM/RM operations, but they are not interchangeable wire encodings. V1 Seata-Rust targets the native Seata protocol path, not the Go-specific Protobuf representation.

## 2. Getty request lifecycle

Pinned sources:
- [Getty client](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/remoting/getty/getty_client.go)
- [Getty remoting](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/remoting/getty/getty_remoting.go)

The sync path is:

1. Allocate a monotonically increasing uint32 request ID.
2. Construct RpcMessage with RequestSync, Seata codec, compressor NONE.
3. Select a live Getty session.
4. Create MessageFuture and store it by request ID before writing.
5. Call session.WritePkg.
6. If WritePkg returns an error, remove the future and return the write error.
7. If write succeeds, wait in `syncCallback` for either the response future or the 20-second timer.
8. `syncCallback` defers RemoveMessageFuture on both response and timeout paths.

The remoting layer itself does not receive the TM context. The 20-second wait is a transport-local timeout, independent of the transaction context deadline.

### 2.1 Late and duplicate responses

The response processor finds a future by response ID and completes it. If the waiter already timed out and removed the future:

- a late response finds no future and is logged as not found;
- it is not delivered to the original caller;
- it does not reverse the timeout result;
- duplicate response completion is rejected by MessageFuture.Complete and logged.

Therefore a timeout after successful WritePkg is an **unknown remote outcome**, not evidence that TC did not execute the request.

### 2.2 Write failure versus wait timeout

| Failure point | Source-observed result | What can be concluded |
|---|---|---|
| no session / closed session before send | immediate error | request not written by this call |
| WritePkg returns error | future removed; error returned | local transport did not report successful write; exact partial-write behavior is transport-dependent |
| WritePkg succeeds, no response in 20s | future removed; timeout error | remote may have received and executed request |
| response arrives before timer | future completes; response returned | caller can inspect typed TC response |
| response arrives after timeout cleanup | no future; response dropped from request waiter | original caller remains with unknown outcome |

Do not claim that a failed socket write guarantees zero remote bytes: a transport may fail after partial progress. Only failures known to occur before any write can be treated as definite pre-send failures.

## 3. gRPC request lifecycle

Pinned sources:
- [gRPC client](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/remoting/grpc/grpc_client.go)
- [gRPC remoting](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/remoting/grpc/grpc_remoting.go)

The gRPC implementation follows the same broad lifecycle:
- allocate request ID;
- create and store MessageFuture;
- encode Protobuf;
- send on a selected stream/channel;
- remove future on encode/send error;
- wait up to 20 seconds;
- remove future after response or timeout;
- late response after cleanup is not delivered to the original waiter.

This shared correlation pattern does not make the gRPC payload Java native V1 compatible.

## 4. TM operation retry policy is above transport

Pinned sources:
- [Getty TM](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/tm/transaction/getty/getty_global_transaction.go)
- [gRPC TM](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/tm/transaction/grpc/grpc_global_transaction.go)
- [End-phase normalization](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/tm/transaction/endphase/endphase.go)

| Operation | Go behavior in pinned source | Important implication |
|---|---|---|
| GlobalBegin | one SendSyncRequest; transport error or failed/empty response returns error | no outer begin retry loop in these managers; unknown outcome needs caller reconciliation |
| GlobalCommit | launcher only; if local TM timeout, first routes to Rollback; otherwise retry loop using CommitRetryCount, 100–200 ms backoff | same XID can be sent again after transport error; TC must handle duplicate commit safely |
| GlobalRollback | launcher only; retry loop using RollbackRetryCount, 100–200 ms backoff | repeated rollback request uses same XID |
| Participant Commit/Rollback | ignored | participant must not independently drive global second phase |
| GlobalReport | not implemented in both shown TM transport managers | do not assume Go TM offers this method through these managers |

The backoff retry loop stops when the request context is done or retry count is exhausted. The transport call itself still waits on its own 20-second timeout because SendSyncRequest does not accept the transaction context.

### 4.1 End-phase result precedence

The shared `endphase.Err` helper checks outcomes in this order:

1. A non-nil response with nil send error wins, even if the caller context is now done.
2. Context cancellation/deadline is returned next.
3. Last transport error, annotated with backoff termination.
4. Backoff termination error.
5. Empty response error.

This prevents a response received after the caller deadline from being discarded as a timeout. It does not eliminate unknown outcomes when no response is ever received.

### 4.2 Do not copy Go retry behavior without reviewing Java semantics

Go retries GlobalCommit/GlobalRollback after transport errors. This is a useful engineering reference, not permission to blindly replay every request in Rust. Rust must:
- preserve request identity (XID and operation);
- ensure no accidental cross-operation retry (commit must not become rollback);
- distinguish a decoded TC rejection from a transport timeout;
- reconcile GlobalStatus after retry exhaustion/unknown outcome;
- avoid reissuing GlobalBegin automatically;
- let Java TC remain authoritative for branch/global retry scheduling.

## 5. RM resource registration and reconnect

Pinned sources:
- [RM cache](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/rm/rm_cache.go)
- [Session-open hooks](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/remoting/session_open_hook.go)
- [Getty listener](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/remoting/getty/listener.go)
- [gRPC channel](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/remoting/grpc/channel.go)
- [Client initialization](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/client/client.go)

The Go client registers `ResourceManagerCache.RegisterCachedResources` as a remoting session-open hook. When a new Getty session or gRPC channel is established, the hook walks cached resource managers and their cached resources, then calls RegisterResource again. This is required because TC loses the old resource-to-session association when the connection closes.

The hook aggregates registration errors. Getty's listener logs the error and releases the session; gRPC channel code logs registration errors. The resource cache iterates one resource at a time and contains a TODO to merge resource IDs into one RegisterRMRequest.

**Rust V1 requirement:** keep a durable-in-process registry of resource IDs and branch types, re-register on every new authenticated TC channel/session, and do not mark the RM ready until required registrations succeed. Reconnect must not reuse a stale client ID/channel association.

## 6. RM operation failure semantics

The native Getty RM remoting adapter in the pinned snapshot:
- BranchRegister: sends one sync request; transport error returns error; ResultCode.Failed returns a message error; success returns BranchId.
- BranchReport: sends one sync request; transport error returns error; wrong response type returns a response-fault error; ResultCode.Failed returns message error.
- GlobalLockQuery: sends one sync request; transport error returns error; expected response returns Lockable; unexpected response type currently collapses to false in the helper (a correctness weakness to avoid copying).
- RegisterResource: sends RegisterRMRequest; transport error returns error; Identified decides registration success/failure callbacks. The shown method logs failure callback but returns nil, so caller-visible failure propagation needs special scrutiny.

These are source observations, not recommendations to copy every behavior. Rust should improve response-type and registration-failure handling while preserving Java protocol semantics.

## 7. V1 Rust request state machine

For each request ID:

```text
Created
  -> Encoded
  -> RegisteredPending
  -> WriteAttempted
      -> DefiniteLocalFailure (only if known not sent)
      -> AwaitingResponse
          -> ResponseReceived
          -> TimedOutUnknown
          -> ConnectionClosedUnknown
ResponseReceived after TimedOutUnknown -> LateResponseObserved (not original completion)
Duplicate response -> DuplicateIgnored + metric
```

A timeout must not delete the only durable business correlation data (XID/BranchId). Request-future cleanup is transport bookkeeping; transaction reconciliation is TM/RM state management.

## 8. Required transport acceptance tests

| ID | Test | Acceptance |
|---|---|---|
| NET-001 | connect refused before write | typed pre-send transport error |
| NET-002 | write succeeds, response withheld beyond timeout | UnknownOutcome, pending future cleaned |
| NET-003 | response arrives just before timeout | exactly one completion |
| NET-004 | response arrives after timeout | late response ignored/counted; no future leak |
| NET-005 | duplicate response | no double completion or state mutation |
| NET-006 | connection closes while waiting | UnknownOutcome if write may have occurred |
| NET-007 | request ID wrap/collision | no in-flight collision; generation strategy documented |
| NET-008 | reconnect after RM registration | all resource IDs re-registered before ready |
| NET-009 | partial registration failure | RM remains degraded/not-ready and exposes per-resource errors |
| NET-010 | commit retry after response loss | same XID, safe duplicate request, status reconciliation |
| NET-011 | begin response loss | no blind second begin |
| NET-012 | branch register response loss | reconcile global/branch state before retrying registration |

No runtime tests are claimed by this source audit.
