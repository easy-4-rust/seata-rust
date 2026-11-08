# Phase 0.9 — Live Java TC Integration Validation Plan

> **Current substage:** 0.9.1 — isolated Java TC FILE bootstrap.
>
> **Boundary:** no production Seata-Rust TM/RM implementation is added here. The harness first proves that the pinned Java TC can be built and started reproducibly. TCP reachability is not protocol interoperability.

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

## 4. INT test sequence after bootstrap

| ID | Test | Required behavior | Status |
|---|---|---|---|
| INT-ENV-001 | Build pinned Java TC | Executable server artifact created | Pending CI |
| INT-ENV-002 | Start FILE TC | Process remains alive and binds configured port | Pending CI |
| INT-ENV-003 | Stop TC | Graceful shutdown releases port | Pending CI |
| INT-ENV-004 | Reset isolated storage | Refuses while running; resets only dedicated data directory | Scripted; runtime pending |
| INT-001 | RegisterTM over native V1 | Identified=true from live TC | Not implemented |
| INT-002 | RegisterRM over native V1 | Identified=true and resource association established | Not implemented |
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

## 5. Required independent client behavior

The future test client must use two independently registered logical clients/connections:

- TM connection: RegisterTM, GlobalBegin, GlobalStatus, GlobalCommit/GlobalRollback.
- RM connection: RegisterRM, receive TC BranchCommit/BranchRollback requests, send typed responses.

The RM listener must remain alive while the TM issues second-phase requests. A one-way request-only client cannot validate a real transaction lifecycle.

Before Rust test-client code is introduced, protocol and transport fixtures remain the reference for V1 frame and payload bytes. The integration client must not invent new encodings.

## 6. Evidence labels

- **Verified in source:** pinned Java source/configuration path confirms behavior.
- **Verified by test:** GitHub Actions or local command passed and run ID is recorded.
- **TCP smoke only:** process starts and socket accepts connections; no Seata message exchanged.
- **Unknown:** no live request/response evidence yet.

A successful INT-ENV smoke does not close INT-001–INT-014.

## 7. Exit gate to WP-01

Phase 0.9.1 passes when INT-ENV-001–004 are verified in CI and the TC can be repeatedly built, started, stopped and reset from a clean runner.

Phase 0.9 as a whole remains open until live RegisterTM/RegisterRM, global transaction lifecycle, RM callbacks and at least FILE restart behavior are tested. Only then should WP-01 be treated as unblocked by live-TC evidence.
