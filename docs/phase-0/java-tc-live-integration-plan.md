# Phase 0.9 — Live Java TC Integration Validation Plan

> **Current substage:** 0.9.2 — independent Rust registration client.
>
> **Boundary:** no production Seata-Rust TM/RM implementation is added here. The standalone test client currently covers only V1 RegisterTM/RegisterRM; transaction lifecycle and phase-two callback handling remain separate gates.

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
| INT-003 | GlobalBegin | TC returns valid XID | Not implemented |
| INT-004 | BranchRegister | TC returns BranchId and stores branch | Not implemented |
| INT-005 | GlobalStatus | TC returns authoritative status | Not implemented |
| INT-006 | GlobalCommit | TC issues BranchCommit to registered RM | Not implemented |
| INT-007 | RM BranchCommit response | TC accepts response and advances branch/global state | Not implemented |
| INT-008 | GlobalRollback | TC issues BranchRollback to registered RM | Not implemented |
| INT-009 | RM BranchRollback response | TC accepts response and advances branch/global state | Not implemented |
| INT-010 | Duplicate phase-2 instruction | RM callback and TC outcome are idempotent | Not implemented |
| INT-011 | FILE restart after Begin | Persisted session behavior is observed | Not implemented |
| INT-012 | FILE restart after BranchRegister | Global and branch records are observed after reload | Not implemented |
| INT-013 | Response loss after BranchRegister | Unknown outcome reconciled without duplicate branch | Not implemented |
| INT-014 | Response loss after Commit/Rollback | Query/retry follows TC authoritative state | Not implemented |

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

- **Verified in source:** pinned Java source/configuration path confirms behavior.
- **Verified by test:** GitHub Actions or local command passed and run ID is recorded.
- **TCP smoke only:** process starts and socket accepts connections; no Seata message exchanged.
- **Unknown:** no live request/response evidence yet.

A successful INT-ENV smoke does not close transaction lifecycle gates INT-003–INT-014. REG-001/REG-002 are separately verified only when the Rust client workflow passes against the running TC.

## 7. Exit gate to WP-01

Phase 0.9.1 bootstrap gates INT-ENV-001–003 passed in Run #15; the stopped-state storage reset ran successfully. The negative reset-while-running case remains pending. REG-001/REG-002 passed in Run #23 with auth disabled.

Phase 0.9 as a whole remains open until live RegisterTM/RegisterRM, global transaction lifecycle, RM callbacks and at least FILE restart behavior are tested. Only then should WP-01 be treated as unblocked by live-TC evidence.
