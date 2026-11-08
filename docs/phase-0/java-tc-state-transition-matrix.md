# Java Seata 2.x — Global / Branch State Transition & Recovery Matrix

> **Status: source-derived behavioral matrix for the pinned Java snapshot.**
>
> Baseline: `apache/incubator-seata` commit `cba73e180fd8896a99df6415b859d275122feaa7` (2.x snapshot used in Phase 0).
>
> This document distinguishes (a) explicit state mutation observed in source, (b) coordinator decisions that lead to a state, and (c) recovery behavior on restart. It is not a claim that every possible pair of enum values is a legal transition.

## 1. Authority and core source files

| Concern | Source |
|---|---|
| Global status code and helper predicates | [GlobalStatus.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/common/src/main/java/org/apache/seata/core/model/GlobalStatus.java) |
| Branch status code | [BranchStatus.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/model/BranchStatus.java) |
| Global status family validation | [SessionStatusValidator.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/session/SessionStatusValidator.java) |
| Commit/rollback coordinator logic | [DefaultCore.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java) |
| Retry / timeout scheduler | [DefaultCoordinator.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/coordinator/DefaultCoordinator.java) |
| End-state and failure helpers | [SessionHelper.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/session/SessionHelper.java) |
| Restart recovery | [SessionHolder.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/session/SessionHolder.java) |
| Session and branch registration | [GlobalSession.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/session/GlobalSession.java), [BranchSession.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/session/BranchSession.java) |

## 2. GlobalStatus: complete code table

The Java enum assigns explicit codes 0–20. Preserve these codes at the Java protocol boundary; do not rely on Rust enum discriminants.

| Code | Status | Operational meaning | Classification |
|---:|---|---|---|
| 0 | UnKnown | Ambiguous/pre-begin or unknown session state | sentinel |
| 1 | Begin | Phase 1; accepts new branch registration | active |
| 2 | Committing | Commit phase is running | active |
| 3 | CommitRetrying | Recoverable commit failure scheduled for retry | retry |
| 4 | Rollbacking | Rollback phase is running | active |
| 5 | RollbackRetrying | Recoverable rollback failure scheduled for retry | retry |
| 6 | TimeoutRollbacking | Timeout detector closed the session and started rollback | active timeout |
| 7 | TimeoutRollbackRetrying | Timeout rollback retry | retry timeout |
| 8 | AsyncCommitting | AT asynchronous branch cleanup continues; externally may be reported as committed | async |
| 9 | Committed | Commit completed | terminal success |
| 10 | CommitFailed | Commit cannot continue automatically; manual handling | terminal failure |
| 11 | Rollbacked | Rollback completed | terminal success |
| 12 | RollbackFailed | Rollback cannot continue automatically; manual handling | terminal failure |
| 13 | TimeoutRollbacked | Timeout-triggered rollback completed | terminal success |
| 14 | TimeoutRollbackFailed | Timeout-triggered rollback failed | terminal failure |
| 15 | Finished | Session is absent/finished; also used by global report semantics | sentinel/result |
| 16 | CommitRetryTimeout | Commit retry deadline exhausted | terminal failure |
| 17 | RollbackRetryTimeout | Rollback retry deadline exhausted | terminal failure |
| 18 | Deleting | Session cleanup/deletion in progress | cleanup |
| 19 | StopCommitOrCommitRetry | Operator has stopped commit/retry | operator stop |
| 20 | StopRollbackOrRollbackRetry | Operator has stopped rollback/retry | operator stop |

### 2.1 Source helper predicates are not interchangeable

The pinned source has several distinct classifications:

| Predicate | Included statuses |
|---|---|
| `GlobalStatus.isOnePhaseTimeout` | TimeoutRollbacking, TimeoutRollbackRetrying, TimeoutRollbacked, TimeoutRollbackFailed |
| `GlobalStatus.isTwoPhaseSuccess` | Committed, Rollbacked, TimeoutRollbacked, Deleting |
| `GlobalStatus.isTwoPhaseHeuristic` | Finished |
| `SessionStatusValidator.isTimeoutGlobalStatus` | TimeoutRollbacked, TimeoutRollbackFailed, TimeoutRollbacking, TimeoutRollbackRetrying |
| `SessionStatusValidator.isTimeoutRollbacking` | TimeoutRollbacking, TimeoutRollbackRetrying |
| `SessionStatusValidator.isRollbackGlobalStatus` | Rollbacking, RollbackRetrying, Rollbacked, RollbackFailed, RollbackRetryTimeout |
| `SessionStatusValidator.isCommitGlobalStatus` | Committing, AsyncCommitting, CommitRetrying, Committed, CommitFailed, CommitRetryTimeout |
| `SessionStatusValidator.isEndGlobalStatus` | Rollbacked, TimeoutRollbacked, Committed, Finished |

Notable discrepancy: `GlobalStatus.isTwoPhaseSuccess` includes Deleting, while `SessionStatusValidator.isEndGlobalStatus` does not. Do not reduce these helpers to one Rust `is_terminal()` without preserving each call-site meaning.

### 2.2 Cross-family status validation

`SessionStatusValidator.validateUpdateStatus(before, after)` rejects four cross-family changes:

| Before family | After family | Result |
|---|---|---|
| Timeout rollback family | Commit family | rejected |
| Commit family | Timeout rollback family | rejected |
| Rollback family | Commit family | rejected |
| Commit family | Rollback family | rejected |

The validator does not define a complete whitelist of all same-family transitions. It prevents a transaction from switching between mutually exclusive commit/rollback directions.

## 3. Global transaction transition matrix

This matrix records the observed normal coordinator path and the source location responsible for it.

| Current state | Trigger / condition | Next state / outcome | Source behavior |
|---|---|---|---|
| UnKnown | New session created | Begin | GlobalSession constructor initializes Begin |
| Begin | TM GlobalCommit, not timed out, synchronous branch work required | Committing | DefaultCore.commit closes session then changes status |
| Begin | TM GlobalCommit and all current branches are async-committable | AsyncCommitting, externally returned as Committed | DefaultCore.commit calls asyncCommit; cleanup may continue |
| Committing | All synchronous commit work succeeds and no remaining branches | Committed | DefaultCore.doGlobalCommit → SessionHelper.endCommitted |
| Committing | Commit work succeeds and remaining branches are async-committable | AsyncCommitting; TM receives Committed view | DefaultCore.commit |
| Committing | Retryable branch commit failure | CommitRetrying | DefaultCore.doGlobalCommit queues retry |
| Committing / CommitRetrying | Unretryable branch commit failure | CommitFailed | SessionHelper.endCommitFailed |
| CommitRetrying | Retry succeeds | Committed | retry scheduler calls doGlobalCommit(retrying=true), then endCommitted |
| CommitRetrying | MAX_COMMIT_RETRY_TIMEOUT exceeded | CommitRetryTimeout | DefaultCoordinator.handleRetryCommitting |
| Begin | TM GlobalRollback | Rollbacking | DefaultCore.rollback closes session and changes status |
| Rollbacking | All branches roll back successfully | Rollbacked | DefaultCore.doGlobalRollback → SessionHelper.endRollbacked |
| Rollbacking | Retryable branch rollback failure | RollbackRetrying | DefaultCore.doGlobalRollback queues retry |
| Rollbacking / RollbackRetrying | Unretryable rollback failure | RollbackFailed | SessionHelper.endRollbackFailed |
| RollbackRetrying | Retry succeeds | Rollbacked | retry scheduler calls doGlobalRollback(retrying=true) |
| RollbackRetrying | MAX_ROLLBACK_RETRY_TIMEOUT exceeded | RollbackRetryTimeout | DefaultCoordinator.handleRetryRollbacking |
| Begin | TC timeout scanner sees session timed out | TimeoutRollbacking | DefaultCoordinator.timeoutCheck closes session and changes status |
| TimeoutRollbacking | Timeout rollback succeeds | TimeoutRollbacked | SessionHelper.endRollbacked preserves timeout family |
| TimeoutRollbacking | Retryable failure | TimeoutRollbackRetrying | retry rollback scheduler handles timeout family |
| TimeoutRollbacking / TimeoutRollbackRetrying | Unretryable failure | TimeoutRollbackFailed | SessionHelper.endRollbackFailed |
| TimeoutRollbackRetrying | Retry succeeds | TimeoutRollbacked | SessionHelper.endRollbacked |
| TimeoutRollbackRetrying | Retry deadline exhausted | RollbackRetryTimeout | retry timeout branch in DefaultCoordinator |
| AsyncCommitting | Async branch cleanup succeeds | Committed / session end | async scheduler invokes doGlobalCommit(retrying=true) |
| AsyncCommitting | Async cleanup retryable failure | remains eligible for async retry | async scheduler retains session for retry |
| Any commit family | Operator stop | StopCommitOrCommitRetry | administrative stop path; not an automatic success |
| Any rollback family | Operator stop | StopRollbackOrRollbackRetry | administrative stop path |
| Completed / eligible session | Cleanup | Deleting then removed | session cleanup/deletion lifecycle |

### 3.1 Commit path details

1. `DefaultCore.commit(xid)` returns Finished if no GlobalSession exists.
2. If the session is timed out, it returns TimeoutRollbacking rather than starting commit.
3. Under the session lock, only Begin may initiate commit; it closes the session first so no new branch can register.
4. If all branches can commit asynchronously, `asyncCommit()` is selected.
5. Otherwise status becomes Committing and `doGlobalCommit(session, false)` runs.
6. Branches are processed in sorted order. During the first pass, async-committable branches are skipped.
7. PhaseOne_Failed / PhaseOne_PrepareFailed branches are removed; XA PhaseOne_RDONLY branches are ignored on commit.
8. PhaseTwo_Committed branches are removed. PhaseTwo_CommitFailed_Unretryable ends the global transaction as CommitFailed. Other non-success outcomes are retried.
9. When no branches remain, SessionHelper ends the global session as Committed.

### 3.2 Rollback path details

1. `DefaultCore.rollback(xid)` returns Finished if no session exists.
2. Under the session lock it closes the session. Only Begin transitions to Rollbacking and starts rollback work.
3. Branches are processed in reverse registration order.
4. PhaseOne_Failed / PhaseOne_PrepareFailed branches are removed without invoking phase 2.
5. STOP_RETRY branches are skipped during retry.
6. PhaseTwo_Rollbacked branches are removed. PhaseTwo_RollbackFailed_Unretryable ends the global transaction as RollbackFailed or TimeoutRollbackFailed. Other failures are retry candidates.
7. If all branches complete, SessionHelper ends as Rollbacked or TimeoutRollbacked depending on the active timeout status family.

### 3.3 Async AT commit is not equivalent to immediate cleanup completion

The coordinator may expose Committed to TM while the global session is still AsyncCommitting and branch cleanup is pending. Rust clients must not interpret the returned status as proof that every asynchronous cleanup action has already completed. The exact client-visible response and subsequent session cleanup must be tested against the selected Java server.

## 3.4 Exact retry-queue and durable status update methods

The pinned `GlobalSession` implementation makes the retry transitions explicit:

| Method | Current status | Action |
|---|---|---|
| `asyncCommit()` | commit eligible | `changeGlobalStatus(AsyncCommitting)` |
| `queueToRetryCommit()` | StopCommitOrCommitRetry | no-op; operator stop is respected |
| `queueToRetryCommit()` | other retryable commit failure | `changeGlobalStatus(CommitRetrying)` |
| `queueToRetryRollback()` | StopRollbackOrRollbackRetry | no-op; operator stop is respected |
| `queueToRetryRollback()` | TimeoutRollbacking | `changeGlobalStatus(TimeoutRollbackRetrying)` |
| `queueToRetryRollback()` | other retryable rollback failure | `changeGlobalStatus(RollbackRetrying)` |

`GlobalSession.changeGlobalStatus(newStatus)` performs these operations in order:

1. If entering Rollbacking or TimeoutRollbacking, update global lock status to Rollbacking.
2. Call `SessionManager.onStatusChange(session, newStatus)` to persist/store the status change.
3. Only after successful store update, assign the in-memory `session.status`.
4. Notify lifecycle listeners.

This means a store failure must not be treated as a successful in-memory transition. Rust TC must preserve the durable-before-visible state update ordering.

`changeBranchStatus` similarly calls the SessionManager branch-status callback before notifying lifecycle listeners. The branch status itself is maintained on the BranchSession object by the relevant caller.

## 3.5 Operator stop, resume, branch stop and manual deletion

These are explicit administrative state transitions, not automatic retry outcomes.

| Administrative operation | Preconditions | Transition / action | Source |
|---|---|---|---|
| Stop global retry (commit) | status in commit-in-progress/retry set | StopCommitOrCommitRetry | AbstractGlobalService.stopGlobalRetry |
| Stop global retry (rollback) | status in rollback-in-progress/retry set | StopRollbackOrRollbackRetry | AbstractGlobalService.stopGlobalRetry |
| Start global retry (commit) | StopCommitOrCommitRetry | CommitRetrying | AbstractGlobalService.startGlobalRetry |
| Start global retry (rollback) | StopRollbackOrRollbackRetry | RollbackRetrying | AbstractGlobalService.startGlobalRetry |
| Stop branch retry | non-Saga; branch status Unknown, Registered or PhaseOne_Done; global transaction is in a retry/commit/rollback state | BranchStatus.STOP_RETRY | AbstractBranchService.stopBranchRetry |
| Start branch retry | branch status STOP_RETRY; non-Saga | BranchStatus.Registered | AbstractBranchService.startBranchRetry |
| Delete global session | status in failure/retry/finish/stop/deleting allowlist | Deleting; delete branch records; end session | AbstractGlobalService.deleteGlobalSession |
| Force delete global session | administrative force-delete path | Deleting; force-delete branches; end session | AbstractGlobalService.forceDeleteGlobalSession |

The operator APIs do not make a failed branch successful. They pause/resume retries or explicitly delete transaction metadata. These actions require audit logging and permission checks in a future Rust TC.

## 3.6 XA prepare-failure special transition

XA prepare failure is not just an ordinary PhaseOne_Failed branch:

1. Rust/Java XA RM reports BranchStatus.PhaseOne_PrepareFailed.
2. Java XACore.branchReport delegates to the normal branch report path.
3. XACore closes the GlobalSession to prevent new branch registration.
4. If the global status is still Begin, XACore changes it directly to RollbackRetrying.
5. The retry rollback scheduler then drives the global rollback path.

This is why PhaseOne_PrepareFailed must remain distinct from generic PhaseOne_Failed in the Rust branch model.

Source: [XACore.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/transaction/xa/XACore.java).

## 4. Retry and timeout policy

| Mechanism | Scan set | Timeout/failure behavior | Source |
|---|---|---|---|
| Global timeout | Begin only | close session, transition to TimeoutRollbacking | DefaultCoordinator.timeoutCheck |
| Commit retry | CommitRetrying | retry until MAX_COMMIT_RETRY_TIMEOUT; then CommitRetryTimeout | DefaultCoordinator.handleRetryCommitting |
| Rollback retry | RollbackRetrying, TimeoutRollbacking, TimeoutRollbackRetrying | retry until MAX_ROLLBACK_RETRY_TIMEOUT; then RollbackRetryTimeout | DefaultCoordinator.handleRetryRollbacking |
| Async commit retry | AsyncCommitting | retry asynchronous branch completion | DefaultCoordinator.handleAsyncCommitting |
| XAER_NOTA retry special case | XA branch reports XAER_NOTA retryable status | after begin + transaction timeout + max(configured XAER_NOTA retry timeout, transaction timeout), TC treats it as completed for that branch | DefaultCore.isXaerNotaTimeout |
| Branch retry stop | BranchStatus.STOP_RETRY | skip branch during retry pass | DefaultCore.doGlobalCommit/doGlobalRollback |

The retry deadline is measured using the GlobalSession begin time in the inspected coordinator code. Exact defaults and scheduler intervals are configuration-driven and must be recorded from the selected Java server configuration, not hard-coded into the Rust TM/RM.

## 5. BranchStatus: Java code table

| Code | Java BranchStatus | Meaning / TC handling |
|---:|---|---|
| 0 | Unknown | unknown |
| 1 | Registered | TC accepted branch registration |
| 2 | PhaseOne_Done | phase 1 succeeded |
| 3 | PhaseOne_Failed | phase 1 failed; branch removed during phase 2 |
| 4 | PhaseOne_Timeout | no phase-1 report before timeout |
| 5 | PhaseTwo_Committed | phase-2 commit succeeded; branch removed |
| 6 | PhaseTwo_CommitFailed_Retryable | commit retry candidate |
| 7 | PhaseTwo_CommitFailed_Unretryable | global commit fails terminally |
| 8 | PhaseTwo_Rollbacked | phase-2 rollback succeeded; branch removed |
| 9 | PhaseTwo_RollbackFailed_Retryable | rollback retry candidate |
| 10 | PhaseTwo_RollbackFailed_Unretryable | global rollback fails terminally |
| 11 | PhaseTwo_CommitFailed_XAER_NOTA_Retryable | special XA retry/timeout handling |
| 12 | PhaseTwo_RollbackFailed_XAER_NOTA_Retryable | special XA retry/timeout handling |
| 13 | PhaseOne_RDONLY | XA read-only branch; ignored on commit |
| 14 | STOP_RETRY | operator instructs TC to stop retrying this branch |
| 15 | PhaseOne_PrepareFailed | XA prepare failed; branch removed and global rollback proceeds |

### 5.1 Branch lifecycle matrix

| Current branch state | Event | Result |
|---|---|---|
| absent | BranchRegister accepted | Registered |
| Registered | RM reports successful phase 1 | PhaseOne_Done |
| Registered | RM reports phase-1 failure | PhaseOne_Failed |
| Registered | no phase-1 report before timeout | PhaseOne_Timeout |
| Registered | XA prepare fails | PhaseOne_PrepareFailed |
| Registered | XA reports read-only | PhaseOne_RDONLY |
| PhaseOne_Done | TC commit instruction | PhaseTwo_Committed or retry/failure status |
| PhaseOne_Done | TC rollback instruction | PhaseTwo_Rollbacked or retry/failure status |
| PhaseTwo_CommitFailed_Retryable | retry commit instruction | PhaseTwo_Committed or still retryable |
| PhaseTwo_RollbackFailed_Retryable | retry rollback instruction | PhaseTwo_Rollbacked or still retryable |
| retryable branch | operator stop retry | STOP_RETRY |

Branch status is not a single linear enum progression. A branch may be removed from the GlobalSession after success, and mode-specific behavior (AT/XA/TCC/Saga) changes which statuses are meaningful.

## 6. Restart and recovery matrix

The pinned `SessionHolder.reload` implementation applies different handling by storage mode. Recovery is not simply “replay every non-terminal state.”

| Persisted status | Reload behavior observed | Consequence |
|---|---|---|
| Rollbacked / TimeoutRollbacked | calls endRollbacked(retryGlobal=true) | finishes cleanup/removal |
| Committed | calls endCommitted(retryGlobal=true) | finishes cleanup/removal |
| Finished / UnKnown / CommitFailed / RollbackFailed / TimeoutRollbackFailed | removeInErrorState | cleanup/error-state removal; not automatic retry |
| AsyncCommitting / Committing / CommitRetrying | retain for commit retry scheduler; Raft path performs lock cleanup first | resume/retry commit path |
| StopCommitOrCommitRetry / StopRollbackOrRollbackRetry / Deleting | retained in the explicit switch | operator/cleanup state; not automatically rewritten |
| Rollbacking / RollbackRetrying / TimeoutRollbacking / TimeoutRollbackRetrying | branch locks restored; rollback family retained for retry | resume rollback path |
| Begin in Raft reload | older session may be moved to RollbackRetrying; new leader-era session is activated | avoids treating a newly created leader session as stale |
| Begin in non-Raft reload | activated | timeout scanner later decides whether timeout rollback is needed |

For DB/Redis and other storage modes, reload also asynchronously scans selected error/terminal statuses (including UnKnown, Committed, Rollbacked, TimeoutRollbacked and Finished) for cleanup. Do not assume all storage modes have identical reload mechanics.

### 6.1 Recovery invariants to preserve in Rust TC

- Persist the GlobalSession status before treating the transition as durable.
- Restore branch sessions and lock ownership consistently before retrying rollback.
- Do not resume commit and rollback simultaneously.
- Keep operator-stop statuses distinct from retryable failures.
- A missing XID can return Finished; this is not equivalent to a successful commit.
- Retry must be idempotent because the coordinator can crash after sending a branch command but before persisting the response.
- Distinguish “command not delivered”, “command delivered but response lost”, and “branch completed but report lost”.
- Retain enough mode-specific metadata to reconstruct AT Undo Log cleanup, XA recovery and TCC/Saga compensation.

## 7. Known semantic traps for Seata-Rust V1

1. TM commit response Committed may represent asynchronous AT commit acceptance, not completion of all cleanup.
2. Finished means no managed session / ambiguous finished state, not a synonym for Committed.
3. TimeoutRollbacked is distinct from Rollbacked and must remain distinguishable.
4. CommitFailed, RollbackFailed and retry-timeout states require manual/operator handling; do not blindly retry forever.
5. STOP_RETRY is a branch state and operator action, not a terminal global state.
6. XAER_NOTA has explicit bounded retry handling; generic retry logic is insufficient.
7. Recovery behavior depends on storage mode (DB, file, Redis, Raft).
8. Java's status helper predicates are not one universal terminal-state classifier.

## 8. Verification work still required

- [ ] Extract every `changeGlobalStatus` call site and record the exact before/after pair.
- [ ] Extract every `BranchSession.setStatus` call site and record mode-specific behavior.
- [ ] Capture retry scheduler periods and defaults from the chosen Java server config.
- [ ] Verify Deleting and operator-stop transitions from admin/controller methods.
- [ ] Verify Saga's GlobalReport and Finished semantics separately.
- [ ] Run restart/failure tests for DB, file, Redis and Raft session stores.
- [ ] Add model-based transition tests and compare Java's observed transitions with the Rust domain model.

Until these are complete, this is a source-backed transition matrix and recovery map, not a formal proof of the entire Java state machine.
