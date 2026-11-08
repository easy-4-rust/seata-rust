# Java Seata 2.x — Retry-Timeout Recovery and Operator-Intervention Audit

> **Status:** source-derived storage-path audit. No Java test was executed in this session.
>
> Java baseline: `apache/incubator-seata@cba73e180fd8896a99df6415b859d275122feaa7`.

## 1. Executive finding

The two retry-timeout global states are valid operational failure states, not ordinary completed states:

- `CommitRetryTimeout` (code 16)
- `RollbackRetryTimeout` (code 17)

The pinned Java console service explicitly includes both in its failure-state lists and exposes manual retry through `changeGlobalStatus(xid)`:

- CommitRetryTimeout → `DefaultCoordinator.doGlobalCommit(globalSession, true)`
- RollbackRetryTimeout → `DefaultCoordinator.doGlobalRollback(globalSession, true)`

However, the storage-mode recovery paths are asymmetric and do not consistently preserve these states. This is a concrete compatibility and operability risk for a future Rust TC, and a targeted Java regression test should be added before using these states as the Rust TC recovery oracle.

## 2. Source-derived facts

### 2.1 SessionHolder.reload

Pinned source: [SessionHolder.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/session/SessionHolder.java)

The FILE/RAFT reload switch explicitly handles:

- cleanup/removal: Finished, UnKnown, CommitFailed, RollbackFailed, TimeoutRollbackFailed;
- successful finalization: Committed, Rollbacked, TimeoutRollbacked;
- commit recovery: AsyncCommitting, Committing, CommitRetrying;
- operator/cleanup retention: StopCommitOrCommitRetry, StopRollbackOrRollbackRetry, Deleting;
- rollback recovery: Rollbacking, RollbackRetrying, TimeoutRollbacking, TimeoutRollbackRetrying;
- Begin: activated (FILE) or leader-age-sensitive rollback/activation (RAFT).

Neither CommitRetryTimeout nor RollbackRetryTimeout has an explicit case. If either status reaches this switch, it reaches the default unexpected-status path and throws `ShouldNeverHappenException`.

### 2.2 FILE storage has a pre-filter, but it is asymmetric

Pinned source: [FileSessionManager.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/storage/file/session/FileSessionManager.java)

During file log restoration, `checkSessionStatus` returns false for:

`UnKnown, Committed, CommitFailed, Rollbacked, RollbackFailed, TimeoutRollbacked, TimeoutRollbackFailed, RollbackRetryTimeout, Finished`.

**CommitRetryTimeout is missing from this filter.** Therefore:

- RollbackRetryTimeout is discarded from the in-memory session map during FILE restore before SessionHolder.reload receives `allSessions()`.
- CommitRetryTimeout is retained by FileSessionManager, then reaches SessionHolder.reload's default unexpected-status branch.

This is stronger than merely saying both states are absent from the SessionHolder switch: the FILE pre-filter gives them different outcomes.

### 2.3 DB / Redis reload query omits both retry-timeout statuses

Pinned source: [SessionHolder.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/session/SessionHolder.java)

The non-FILE/RAFT asynchronous cleanup query selects:

`UnKnown, Committed, Rollbacked, TimeoutRollbacked, Finished`.

It does not select CommitRetryTimeout or RollbackRetryTimeout. The source therefore does not show these statuses being removed by this cleanup query. Their exact availability through each concrete DB/Redis SessionManager and their manual-retry behavior after restart still require storage-specific tests.

### 2.4 Operator retry explicitly supports both statuses

Pinned sources:

- [AbstractService.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/console/impl/AbstractService.java)
- [AbstractGlobalService.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/console/impl/AbstractGlobalService.java)
- [AbstractGlobalServiceTest.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/test/java/org/apache/seata/server/console/impl/AbstractGlobalServiceTest.java)

`FAIL_COMMIT_STATUS` includes CommitFailed and CommitRetryTimeout. `FAIL_ROLLBACK_STATUS` includes TimeoutRollbacked, RollbackFailed and RollbackRetryTimeout.

`changeGlobalStatus(xid)` dispatches to `doRetryCommitGlobal` or `doRetryRollbackGlobal`. The existing console test verifies that both retry-timeout statuses are accepted by this API. This proves the intended operator capability, but it does not prove that the session is reliably available after a restart in every storage mode.

## 3. Storage-mode behavior matrix

| Store mode | Source-observed handling | Risk / unknown |
|---|---|---|
| FILE + CommitRetryTimeout | FileSessionManager pre-filter does not discard it; SessionHolder.reload has no case | Reaches unexpected-status exception during reload; likely startup/recovery blocker if such a record exists |
| FILE + RollbackRetryTimeout | FileSessionManager.checkSessionStatus filters it out before SessionHolder.reload | Removed from active session map; reconcile this with console manual retry support |
| RAFT + either retry-timeout | SessionHolder.reload switch has no explicit case; no FILE pre-filter in this path | Exact leader reload invocation and persisted record handling require Raft integration test |
| DB + either retry-timeout | asynchronous cleanup query omits both | Intended retention is plausible from console retry API, but restart lookup and retry behavior need DB test |
| Redis + either retry-timeout | same cleanup query omits both | Intended retention is plausible, but restart lookup and retry behavior need Redis test |

Do not translate this into a single Rust rule such as `is_terminal() => delete`. Retry-timeout is a failed transaction state with operator intervention semantics, and storage modes currently differ.

## 4. Existing Java tests and coverage gap

Pinned existing tests:

- `SessionHolderRecoveryUnitTest.terminalErrorsAreRemovedAndStoreFailuresContained` parameterizes Finished, UnKnown, CommitFailed, RollbackFailed and TimeoutRollbackFailed. It does **not** include CommitRetryTimeout or RollbackRetryTimeout.
- `FileSessionManagerTest` does not currently expose a focused test for the retry-timeout pre-filter asymmetry.
- `AbstractGlobalServiceTest` verifies that CommitRetryTimeout and RollbackRetryTimeout are accepted by `changeGlobalStatus`; this tests console validation, not durable restart/reload.
- `CoordinatorRecoveryUnitTest` covers normal retry schedulers, not restart of retry-timeout states.

## 5. Required Java regression tests

These tests should be added to a pinned Java Seata checkout before claiming the state semantics are fully understood.

| ID | Test | Expected assertion |
|---|---|---|
| REC-TIMEOUT-001 | FILE restore with CommitRetryTimeout | No unhandled startup exception; explicitly decide retain-for-operator-retry vs safe cleanup |
| REC-TIMEOUT-002 | FILE restore with RollbackRetryTimeout | Verify current pre-filter behavior and explicitly decide whether deletion is intended |
| REC-TIMEOUT-003 | RAFT leader reload with CommitRetryTimeout | No silent loss; explicit operator-visible retention or documented cleanup |
| REC-TIMEOUT-004 | RAFT leader reload with RollbackRetryTimeout | Same, including lock cleanup and leader transition |
| REC-TIMEOUT-005 | DB restart with CommitRetryTimeout | XID remains queryable and console retry can dispatch commit |
| REC-TIMEOUT-006 | DB restart with RollbackRetryTimeout | XID remains queryable and console retry can dispatch rollback |
| REC-TIMEOUT-007 | Redis restart with both statuses | Same assertions as DB |
| REC-TIMEOUT-008 | Manual commit retry after restart | Branch instructions are replay-safe; status reaches the expected retry/final state |
| REC-TIMEOUT-009 | Manual rollback retry after restart | Branch instructions are replay-safe; lock state and undo/compensation are consistent |
| REC-TIMEOUT-010 | Retry-timeout record with no remaining branches | Operator retry, deletion and cleanup are idempotent |

For every test record:
- persisted status before restart;
- session and branch count after reload;
- lock state after reload;
- console query result;
- retry command result;
- final status and whether the session remains;
- store-specific event/log evidence.

## 6. Rust TC design implications

Until the Java storage behavior is resolved, Rust TC should preserve the distinction between:

- automatic retry states (`CommitRetrying`, `RollbackRetrying`, timeout retry variants);
- retry-timeout states requiring operator intervention;
- operator-stopped states;
- completed states;
- deleted/finished states.

A Rust TC must not automatically delete retry-timeout sessions merely because their retry deadline elapsed. It must also not copy the Java FILE restore asymmetry without an explicit compatibility decision and regression evidence.

For V1 (Rust TM/RM + Java TC), the Rust client must treat CommitRetryTimeout and RollbackRetryTimeout as authoritative TC outcomes and surface the operator-action requirement. It must not start a local retry loop that competes with Java TC.

## 7. Evidence status

| Statement | Evidence |
|---|---|
| Both retry-timeout states are accepted by Java console `changeGlobalStatus` | Verified in pinned source and existing unit test |
| SessionHolder FILE/RAFT reload switch has no explicit case for either | Verified in pinned source |
| FILE pre-filter removes RollbackRetryTimeout but not CommitRetryTimeout | Verified in pinned source |
| DB/Redis cleanup query omits both | Verified in pinned source |
| FILE CommitRetryTimeout causes a production server startup failure in a real persisted deployment | Inferred from call chain; not runtime reproduced |
| DB/Redis sessions remain queryable and manually retryable after restart | Unknown; targeted integration tests required |
| RAFT behavior in a live leader restart | Unknown; targeted integration test required |

**Conclusion:** this is a source-proven inconsistency in recovery-path handling, with a likely FILE-mode CommitRetryTimeout failure and a deliberate-or-accidental RollbackRetryTimeout removal. Do not call it a confirmed production outage until the pinned Java integration tests reproduce it.
