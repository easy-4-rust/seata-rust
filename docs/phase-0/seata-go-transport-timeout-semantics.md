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

### Formal Rust stream and ID ownership decisions (2026-11-13)

Continue the existing WP-02/ADR-0008 contract after the approved WP-01 protocol-component Gate; this is an implementation detail, not a reduction of NET-001–012 or the full transport requirements.

- A stream decoder consumes at most one complete frame per call, advances the caller's input slice and retains only an incomplete frame. The caller can repeatedly drain a coalesced chunk without an unbounded output queue. Protocol-owned prefix validation checks lengths and known capabilities before reserving the advertised body; a valid partial prefix means “need more data”. Malformed frames and EOF terminate that decoder. A fresh connection receives a fresh decoder.
- The shared protocol decoder exposes bounded prefix validation so the transport does not invent a second V1 header grammar. The existing complete-frame decode API retains its strict one-frame/trailing-byte contract.
- Each connection generation exclusively owns one non-clonable ID allocator. It advances through Java signed-int bit patterns, including negative values and zero. IDs are never returned to the allocator within that connection, even after success, timeout, cancellation or a pre-send local failure. Otherwise an arbitrarily delayed duplicate with the same wire ID cannot be distinguished from a new response.
- A configurable per-connection issuance quota in1..2^32 allows earlier rotation and bounded exhaustion tests. Exhaustion requires replacing the connection; it does not authorize replaying a transaction. New-connection generation tagging, pending completion ownership and typed pre-send/unknown outcomes still need their own implementation and tests; an allocator alone does not complete NET-007's generation/in-flight integration, NET-006 or the connection lifecycle.

### Async single-session implementation contract (2026-11-13)

- One owned session manages separate reader/writer tasks and bounded outbound/inbound channels. A process-unique checked connection generation and an independent pending table bind each socket; old readers cannot access a replacement session's table. This is a raw connected channel, not authenticated/RM-ready state.
- Pending registration precedes enqueue/send. An atomic per-request write-attempt marker is set under the pending-table lock immediately before the first write future is polled. No mutex is held across network await. Failures/timeouts before that marker are local pre-send; after it they are UnknownOutcome. The transport never replays a transaction or converts a timeout into a decoded rejection.
- Completion, timeout, cancellation and connection closure share one synchronized remove-and-complete owner. The receiver checks the absolute deadline under that lock, even if the calling future has not polled its timer. A response decided before the deadline is handed off atomically with removal and retains its result if the caller resumes later; a response processed after the deadline produces a classified timeout plus late-response observation. Wrong envelope/type/batch result shape cannot complete a waiter. Unmatched late/duplicate responses are counted and ignored, with no same-connection ID reuse.
- Each call has an explicit transport deadline, separate from the TC business timeout in its payload. Cancellation removes its waiter immediately. A started frame write must finish or close that socket; cancelling a caller must not leave a partial frame followed by another request on a live stream.
- Explicit shutdown joins owned tasks; owner drop signals closure and aborts owned tasks. Task-exit guards close the session even on panic/abort and resolve every pending call according to its write marker. Actual pools, scheduled heartbeat/idle detection, reconnect and authenticated RM re-registration remain subsequent work.

No runtime tests are claimed by this source audit.

### Scheduled heartbeat and reader-idle implementation contract (2026-11-13)

Continue WP-02's required heartbeat/idle task. The pinned Java `AbstractNettyRemotingClient.ClientHandler#userEventTriggered` sends PING on writer idle and invalidates a channel on reader idle; `NettyClientBootstrap` installs its `IdleStateHandler`. Rust keeps those responsibilities with the following explicit scheduling and ownership details, rather than claiming identical default seconds or every Netty runtime detail:

- `ConnectionIdleConfig` explicitly enables positive writer-idle and reader-idle intervals, with reader-idle greater than writer-idle. No implicit heartbeat is added to existing callers that omit it. Both durations must fit checked monotonic deadlines.
- The existing owned writer schedules an empty native V1 outer PING(type3) after no completed outbound frame for the writer-idle interval; there is no extra unsupervised timer task. Complete business writes and PONG replies reset writer idle. Cancelled/skipped queued requests do not reset it. Successful PING writes reset it and increment a separate counter. Delayed timers send one probe, without catch-up bursts.
- Scheduled PINGs consume fresh IDs from the same socket allocator as ordinary requests, but never register ordinary pending futures. Quota exhaustion closes that generation with its existing typed rotation reason; it never reuses an ID or automatically replays a transaction. PONGs(type4) retain control-envelope semantics regardless of ID and cannot complete a transaction waiter. Reader-idle is a channel liveness rule, not a per-PING delivery acknowledgement or proof of authentication/RM readiness.
- The owned reader closes the generation after a full reader-idle interval without a complete valid V1 frame. Valid responses, inbound requests and empty PING/PONG frames reset it; mere TCP fragments do not. This deliberately bounds peers that drip an unfinished frame indefinitely. A frame processed after the absolute idle boundary cannot revive a generation whose read deadline has already expired.
- Reader-idle expiry returns a distinct infrastructure cause and drains all waiters using their individual write-attempt markers. Shutdown, owner Drop, read/write errors and cancellation keep their existing cleanup contract. Closed connections emit no new probes; idle timers share worker shutdown and ownership, and never imply business transaction retries.
- Required verification includes scheduled native PING/PONG, live peers across repeated idle windows, silent-peer expiry with an outstanding request, partial-frame activity that cannot keep the session alive, business/control write resets, ID quota rotation, disabled compatibility and shutdown/Drop cleanup. Monotonic-timer boundary/reset behavior is tested with paused time; actual TCP behavior uses bounded loopback peers separately.

This implements one remaining WP-02 task; pool, reconnect/backoff, authenticated resource re-registration and NET-008–012 remain required separately.

### Bounded raw connection pool and reconnect contract (2026-11-13)

- A pool owns a fixed positive number of per-endpoint supervisors, each owning at most one physical TCP session. Configuration bounds slots (1..1024) and the total forwarded-inbound queue. Multiplexed handles share each socket's existing pending budget; acquiring a handle does not create another TCP connection or an exclusive socket lease.
- Slots expose Connecting, Connected, Backoff and Stopped, plus attempts, generation and last infrastructure cause. Connected means raw TCP only: authentication and cached RM registrations remain a subsequent mandatory initialization layer. No application/transaction request is sent automatically by pool startup or reconnect.
- Each disconnected slot clears its selectable handle, joins/closes its old owned session, waits positive exponentially increasing reconnect delays bounded by an explicit maximum, then creates a fresh generation/decoder/ID allocator. The backoff resets only after a configured stable connection duration, so briefly flapping connections cannot repeatedly defeat backoff. Delay overflow cannot produce zero delay or wrap. Per-endpoint jitter/discovery/TLS and production tuning remain WP-07 work.
- Acquisition observes state changes without lost wakeups, has an absolute caller-supplied deadline, and returns only a currently open generation. A concurrently closed returned handle still fails according to its own write marker; it is never rebound to a replacement. A pool request includes acquisition time in its timeout and invokes exactly one selected session request. It never transparently retries or resubmits a business operation, including when the error is local pre-send.
- Forwarded inbound requests carry their originating session handle/generation. A full pool inbound queue closes that session with Overloaded instead of silently discarding callbacks; already queued messages retain their original generation. Business dispatch and response APIs remain the RM control-plane work.
- Explicit shutdown stops reconnect attempts, terminates all selectable generations, resolves waiters and joins every supervisor. Owner Drop synchronously marks the pool closed, closes generation state and aborts its workers; an exit guard constructed before first poll also closes the pool on unexpected worker termination. Cancelling an acquire/request future cannot leak a socket, create a retry or change supervisor count.
- Verification requires fixed connection count under concurrent acquisition, multiple slots and round-robin selection, acquisition timeout/abort, reconnect with fresh identity and no replay of a lost Begin, continued backoff across short-lived sessions, stable reset, bounded inbound forwarding/overflow and shutdown/Drop cleanup. Paused-time backoff unit checks supplement real TCP acceptance; authentication/RM-ready and NET-008–012 are not inferred from raw pool evidence.

### Origin-bound inbound response and NET-010–012 implementation contract (2026-11-13)

- Replying to a native TC callback requires a non-clonable inbound capability bound to the exact receiving ConnectionHandle/generation. PoolInbound may be consumed once to respond, and a direct TcpConnection can expose the same bound capability separately from its existing raw next_inbound API. An old callback never acquires a replacement pool channel to send its response.
- Only a synchronous request envelope can receive an ordinary response. The reply keeps the original signed request ID and ordered nullable headers, validates expected response type/complete flat batch shape, and preserves exact XID/BranchId for branch-end messages. A one-way/control/result envelope, wrong response family, incomplete batch or changed branch identity fails locally before any response write.
- Outbound response receipts are separate from ordinary request futures: TC request IDs can equal locally allocated IDs without collision. A bounded response-write budget and existing bounded writer queue prevent unbounded replies; ordinary waiters do not consume the dedicated response budget. The same owned writer performs complete frame writes and no mutex spans network await.
- A response call returns success only after its entire local frame write completed within its absolute deadline. This is not a TC acknowledgement or durable business completion. Errors before first write poll are local; after a write may have begun they are unknown. Deadline/cancellation before writing causes the queued reply to be skipped; after writing begins, the frame must finish or the socket must close, never partial-frame reuse. Worker/queue drop and closure resolve receipts once with the write marker intact.
- Replying to one received message does not implement business idempotency for independently redelivered TC callbacks. The RM layer must validate resource/branch/application identity and keep durable completion/transaction state; test-only simulated branch actions are not SQL/Undo compensation.
- NET-011 must show one actual Begin request, a successfully executed response lost outside the client, an unknown outcome with no returned XID and no blind second Begin. An observer's captured XID is not fed into normal client recovery.
- NET-010 must show the original XID retained after a lost Commit response, explicit status observation before any caller-controlled same-operation duplicate, no operation change to Rollback and no repeated branch effect. A Finished/no-record observation alone must not rewrite the original unknown Commit as a definite historical success. The transport itself never retries the operation.
- NET-012 must show no duplicate BranchRegister after response loss. A test-only pending branch may reconcile its exact original identity from a legitimate matching TC rollback callback after explicit cancellation of the same XID; the observer's dropped-response BranchId is used only as an independent oracle, never supplied to the client. Production durable RM recovery remains a later gate.

These are formal transport/fault-driver requirements, not permission to weaken TM/RM/MySQL, production authentication, WP00-R or any external-TC/shared-lock boundary.
