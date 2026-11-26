# Phase 0.9 — Live Java TC Integration Validation Plan

> **Current substage:** 0.9.2 — independent Rust client; registration, simulated AT callbacks, callback redelivery, normal FILE restart and three externally lost-response scenarios verified locally through 2026-11-12.
>
> **Boundary:** no production Seata-Rust TM/RM implementation is added here. Coverage includes callback/mutation-response loss, DB/Redis seeded recovery, bounded auth-hook/reconnect and real RAFT election/snapshot diagnostics. Finished alone retains uncertainty. Database/Undo Log compensation, arbitrary duplicate/reconnect, production authentication, safe RAFT timeout recovery/operator retry and broader recovery remain open. See [local validation results](phase09-local-validation-results.md).

## 1. Frozen runtime topology

| Component | Setting |
|---|---|
| Java TC source | `apache/incubator-seata@cba73e180fd8896a99df6415b859d275122feaa7` |
| JDK | 17 |
| TC storage | FILE |
| Global / branch session store | FILE |
| Lock store | FILE |
| Config center | file |
| Registry | file |
| Native service port | 18091 (override with `SEATA_TC_PORT`) |
| Isolated workspace | `$SEATA_TC_WORK_DIR`, default `$TMPDIR/seata-rust-java-tc` |
| Authentication | disabled for bootstrap smoke only |

The configuration is in [application.yml](../../tools/java-tc-integration/config/application.yml). Authentication is disabled because this substage verifies server bootstrap and socket reachability only. RegisterTM/RegisterRM auth behavior must be tested separately before enabling or claiming auth compatibility.

## 2. Reproducible commands

From the repository root:

```bash
bash tools/java-tc-integration/scripts/build-tc.sh
bash tools/java-tc-integration/scripts/start-tc.sh
bash tools/java-tc-integration/scripts/stop-tc.sh
bash tools/java-tc-integration/scripts/reset-storage.sh
bash tools/java-tc-integration/scripts/smoke.sh
```

The scripts:
1. Clone the pinned Java source if absent.
2. Fetch and checkout the exact commit SHA.
3. Build `server` and required reactor modules.
4. Start the Spring Boot executable with the isolated FILE config.
5. Wait for the configured TCP listener.
6. Stop the process and preserve logs/data unless explicitly reset.

The default work directory is under the OS temporary directory. `reset-storage.sh` refuses to run while the TC PID is alive and only removes the dedicated `data` child directory. Do not point `SEATA_TC_WORK_DIR` at a valuable directory.

## 3. Automation

[Phase 0.9 Java TC FILE Bootstrap](../../.github/workflows/phase-0-java-tc-file-smoke.yml) runs the build/start/TCP smoke in GitHub Actions on changes to the integration harness, on pull requests, and manually.

The workflow result must be recorded with:
- Seata-Rust commit SHA;
- Java Seata commit SHA;
- Maven build result;
- server process startup result;
- TCP listener result;
- log artifact on failure.

### 3.1 Bootstrap runtime result

[GitHub Actions Run #15](https://github.com/easy-4-rust/seata-rust/actions/runs/42474316751) passed on 2026-11-08 using Java Seata commit `cba73e180fd8896a99df6415b859d275122feaa7`.

Verified in that run:
- Maven built the self-contained `server/target/seata-server.jar` with the pinned `release-seata-jar` profile.
- The Java TC process started with the isolated FILE configuration.
- The native TCP listener accepted connections on `127.0.0.1:18091`.
- The smoke script stopped the Java TC process successfully.
- The isolated storage reset ran before startup.

This was a **TCP bootstrap smoke only**. No Seata message was exchanged in Run #15. The build script retries Maven up to three times because a prior run encountered a Maven Central HTTP 504 while resolving Kotlin compiler 2.2.20.

## 4. INT test sequence after bootstrap

| ID | Test | Required behavior | Status |
|---|---|---|---|
| INT-ENV-001 | Build pinned Java TC | Executable server artifact created | Verified by test, Run #15 |
| INT-ENV-002 | Start FILE TC | Process remains alive and binds configured port | Verified by test, Run #15 |
| INT-ENV-003 | Stop TC | Graceful shutdown releases port | Verified by test, Run #15 |
| INT-ENV-004 | Reset isolated storage | Resets dedicated data directory while stopped | Verified by test, Run #15; refusal-while-running negative case pending |
| INT-001 | RegisterTM over native V1 | Identified=true from live TC | Verified by test, Run #23; auth disabled |
| INT-002 | RegisterRM over native V1 | Identified=true and resource association established | Verified by test, Run #23; auth disabled |
| INT-003 | GlobalBegin | TC returns valid XID | Verified locally 2026-11-11, two independent empty transactions |
| INT-004 | BranchRegister | TC returns BranchId and stores branch | Verified locally 2026-11-12, simulated AT branches; disk recovery not tested |
| INT-005 | GlobalStatus | TC returns authoritative status | Verified locally 2026-11-11, Begin then Finished after successful empty commit/rollback |
| INT-006 | GlobalCommit | TC issues BranchCommit to registered RM | Verified locally 2026-11-12, simulated AT branch |
| INT-007 | RM BranchCommit response | TC accepts response and advances branch/global state | Verified locally 2026-11-12, PhaseTwo_Committed; server receipt/completion logs |
| INT-008 | GlobalRollback | TC issues BranchRollback to registered RM | Verified locally 2026-11-12, simulated AT branch |
| INT-009 | RM BranchRollback response | TC accepts response and advances branch/global state | Verified locally 2026-11-12, PhaseTwo_Rollbacked; server receipt/completion logs |
| INT-010 | Duplicate phase-2 instruction | RM callback and TC outcome are idempotent | Partial: local 2026-11-12, one withheld response/redelivery per simulated AT branch; process-local success deduplication only |
| INT-011 | FILE restart after Begin | Persisted session behavior is observed | Local 2026-11-12 passed: original XID Begin after forced TC process restart |
| INT-012 | FILE restart after BranchRegister | Global and branch records are observed after reload | Local 2026-11-12 passed: original BranchId callback; lock false before/after restart, true after rollback |
| INT-013 | Response loss after BranchRegister | Unknown outcome reconciled without duplicate branch | Local 2026-11-12 passed for one pending simulated branch: no resend; validated nonce callback learns original BranchId, rollback releases lock |
| INT-014 | Response loss after Commit/Rollback | Query/retry follows TC authoritative state | Local 2026-11-12 passed for one external loss each: no end resend; Finished15 keeps outcome unknown; callback/lock cleanup independently verified |

## 5. Independent Rust test client

The first independent Rust client now exists under `tools/java-tc-integration/rust-client` and uses only the Rust standard library; it is not a production crate.

Current behavior:
- Builds Seata V1 16-byte frame headers and Seata serializer payloads.
- Opens independent TCP connections for TM and RM.
- Sends RegisterTM (type 101) and RegisterRM (type 103).
- Reads partial-safe V1 response frames, correlates request IDs, validates protocol/serializer/compressor fields, and parses IdentifyResponse (types 102/104).
- Requires `identified=true`; preserves both connections briefly after registration.
- Unit tests check V1 header fields and the RM string32 ResourceIds encoding.

Runner: `bash tools/java-tc-integration/scripts/run-rust-registration.sh`.

The FILE smoke script starts Java TC and runs this Rust client before stopping TC. The CI workflow installs stable Rust. Run #23 has verified live RegisterTM/RegisterRM with auth disabled. The client is now being aligned to the pinned Go client's version string (1.1.0), and the FILE data directory has been corrected to sit under the resettable `data/` path.

The next client increment must keep the RM connection alive in a receive loop, handle BranchCommit/BranchRollback callbacks, and respond with the exact Java-compatible message types. Only then can TM GlobalCommit/GlobalRollback be considered end-to-end tests.

### 5.1 Live registration result — Run #23

[GitHub Actions Run #23](https://github.com/easy-4-rust/seata-rust/actions/runs/42475353541) passed on 2026-11-08.

The workflow built the pinned Java TC, started FILE storage, and ran the independent Rust client. Observed responses:
- `RegisterTMResponse`: `identified=true`, protocol V1, server version `2.8.0-SNAPSHOT`.
- `RegisterRMResponse`: `identified=true`, protocol V1, resource ID `mysql://orders`, server version `2.8.0-SNAPSHOT`.
- Rust client unit tests: 2 passed.
- Java TC stopped cleanly after the registration smoke.

The client initially used the version string from the Go codec test fixture. It is being aligned to the actual pinned Seata-Go client constant `constant.SeataVersion = "1.1.0"` for subsequent runs. The bootstrap config still has auth checks disabled; this proves the unauthenticated registration path only.

## 6. Evidence labels

### Local increment — 2026-11-11

The existing isolated Rust probe now supports `--lifecycle`. A pinned Java TC built successfully with a temporary Maven Central configuration and completed two separate no-branch transactions on port 28091: Begin → Status(Begin) → Commit(Committed=9) → Status(Finished=15), and Begin → Status(Begin) → Rollback(Rollbacked=11) → Status(Finished=15). TM and RM both registered using client version `1.1.0`. See [raw evidence](evidence/local-phase09-live/README.md).

This extends the earlier registration-only implementation described above. It does not close INT-006–009: those gates specifically require real RM phase-two callbacks. Windows process termination released the port, but was forced; graceful shutdown and durable FILE recovery remain unverified.

### Local increment — 2026-11-12

`--branches` now registers two simulated AT branches in distinct TC-issued XIDs, services the real TC callbacks concurrently with TM end requests, returns matching response IDs and branch statuses, then queries Finished. Both paths passed with the same pinned TC JAR, and TC logs show receipt plus successful processing of the branch responses. [Evidence](evidence/local-phase09-branches/README.md). INT-004 and INT-006–009 pass for this simulated-resource scope; no database AT/Undo Log or recovery claim follows. Final Rust suite: 21 tests passed; fmt and Clippy exit 0.

- **Verified in source:** pinned Java source/configuration path confirms behavior.
- **Verified by test:** GitHub Actions or local command passed and run ID is recorded.
- **TCP smoke only:** process starts and socket accepts connections; no Seata message exchanged.
- **Unknown:** no live request/response evidence yet.

A successful INT-ENV smoke does not close transaction lifecycle gates INT-003–INT-014. REG-001/REG-002 are separately verified only when the Rust client workflow passes against the running TC.

## 7. Exit gate to WP-01

### Callback response-loss increment — 2026-11-12

`--callback-loss` withholds the first successful simulated RM response and requires real TC redelivery with a new RPC ID. Commit and rollback passed, with two deliveries and one simulated action per branch. The rollback initial response was Failed/RollbackRetrying=5; the probe logs that outcome without replay and continues only in this explicit fault mode. Later status queries return Finished and TC logs confirm successful branch response processing. [Evidence and limits](evidence/local-phase09-callback-loss/README.md). INT-010 is partially covered; this is not TM/BranchRegister response-loss or durable idempotency evidence.

Phase 0.9.1 bootstrap gates INT-ENV-001–003 passed in Run #15; the stopped-state storage reset ran successfully. The negative reset-while-running case remains pending. REG-001/REG-002 passed in Run #23 with auth disabled.

### External mutation-response loss increment — 2026-11-12

The owned proxy discards a complete successful type12/8/10 response in three separate live cases. Client requests are sent once; original BranchId is learned from a validated callback after registration loss. For lost Commit/Rollback the only later observed end status is Finished15, so uncertainty is retained, as required by approved ADR-0008. Observer traffic is never supplied to the client. Both default and spaced runtime paths passed; final Rust93/93, Python5/5. [Evidence and reproduction](evidence/local-phase09-response-loss/README.md). INT-013–014 pass for this bounded same-connection simulated scope; business outcome resolution from Finished is not claimed.

Phase 0.9 remains open. Normal FILE Begin/Registered records have process-restart evidence in [local-phase09-restart](evidence/local-phase09-restart/README.md). Broader loss/reconnect, retry-timeout and store-specific recovery remain distinct gates. This does not prove the full Phase 0–7 plan or production components complete.

### Native authentication and cached-resource reconnect — 2026-11-12

Three fresh pinned TC processes pass the default-permissive, external-test-checker-enabled and checker-bypassed controls. The enabled fixture rejects16 requests across TM/RM credentials, identity/group and RM resource boundaries; every same-channel transaction stimulus closes without allocating FILE global data. A valid client creates two original branches, closes its old RM, authenticates a fresh RM with both cached resource IDs, and services the original commit/rollback callbacks. The runner first observes the old-channel removal; both callbacks and TC completion logs match, followed by Finished. Rust103 tests and focused Java7 pass. [Evidence](evidence/local-auth-registration/README.md); [reproduction](../../tools/java-tc-integration/auth_validation/README.md).

REG-003–006 pass only within this simulated-resource scope. The fixed public checker does not establish a production policy. The default provider still allows all, and original registration logs expose extraData; [production boundaries](auth-runtime-boundaries.md) remain explicit.

### Real RAFT leader/snapshot diagnostics — 2026-11-12

Three fresh three-node cohorts complete six real leader transitions and three fresh-process snapshot reloads. Original16/17 records/branches/locks remain queryable through the actual console and TM-only native V1 probe, but four current-transition JFR stacks prove failed original async reload. Begin controls queue rollback/complete as documented. Java4, Rust106 and Python3 pass as diagnostic checks; safe recovery acceptance remains open. [Evidence](evidence/local-raft-retry-timeout/README.md), [runtime findings](raft-reload-runtime-findings.md), [reproduction](../../tools/java-tc-integration/raft_recovery/README.md). No original mutation replay, natural deadline or real RM compensation is claimed.
