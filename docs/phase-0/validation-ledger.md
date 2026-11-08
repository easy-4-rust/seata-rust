# Seata-Rust Phase 0 — Compatibility and Integration Validation Ledger

> **Current status:** Go codec/frame vectors and selected Java V1/TC unit tests have passed in GitHub Actions. Phase 0.9 Java TC FILE bootstrap harness is committed; its CI execution is pending. No live Java TC Seata request/response interoperability is claimed yet.

## 1. Frozen source baselines

| Upstream | Commit | Purpose |
|---|---|---|
| Apache Seata Java 2.x | `cba73e180fd8896a99df6415b859d275122feaa7` | V1 frame decoder/encoder, serializer and TC semantic authority |
| Apache Seata-Go v2 | `82570d905919c1c66bb025a3a5979f25390de498` | Native Seata codec and primary V1 client execution reference |
| RSeata | `c6062776c72a2490f62f8ebb25012c60884f62bd` | Third-party Rust abstraction reference only |

## 2. Protocol runtime evidence

### Run #8 — Go and Java V1 protocol tests

[GitHub Actions Run #8](https://github.com/easy-4-rust/seata-rust/actions/runs/42457053815), Seata-Rust commit `756a34a9e0198b0d6b2d6a7cafe9f404a2109804`.

- Go job passed 29 codec payload vectors and 4 complete V1 frames against Go commit `82570d9`.
- Java job passed 4 V1 Decoder → Encoder complete-frame byte-identical round trips against Java commit `cba73e1`.
- This proves the tested Go codec/serializer fixtures and four Java V1 frame cases. It does not prove all Java message codecs or a live Java TC transaction.

### Run #18 — Java TC inbound exception tests

[GitHub Actions Run #18](https://github.com/easy-4-rust/seata-rust/actions/runs/42458368199).

- 8 selected `AbstractTCInboundHandler` exception tests passed.
- Together with 4 V1 frame tests, Maven reported 12 tests, 0 failures, 0 errors, 0 skipped.
- Covered GlobalBegin TransactionException/RuntimeException/StoreException, GlobalCommit status fallback, GlobalStatus status fallback, GlobalReport request-status preservation and BranchRegister StoreException.
- Remaining GlobalRollback, BranchReport, GlobalLockQuery, RegTM/RM auth and status-fallback lookup failure cases remain open.

### Run #25 — Java retry-timeout FILE/RAFT reload unit tests

[GitHub Actions Run #25](https://github.com/easy-4-rust/seata-rust/actions/runs/42458877018).

- 3 retry-timeout unit tests passed.
- Together with the 12 earlier tests, Maven reported 15 tests, 0 failures, 0 errors, 0 skipped.
- Verified FILE pre-filter asymmetry and SessionHolder FILE/RAFT reload branches.
- This does not reproduce a full persisted FILE restart, DB/Redis restart, or live Raft leader recovery.

## 3. Fixture inventory

| Asset | Coverage | Evidence |
|---|---|---|
| `tools/go-golden-vectors/expected.json` | 29 codec-body + serialized-payload vectors (25 base messages + 4 failure responses) | Go runtime verified in Run #8 |
| `tools/go-golden-vectors/frames.json` | 4 full V1 frames: begin request, begin response, optional head map, failed begin response | Go runtime verified in Run #8 |
| `tools/go-golden-vectors/main.go` | Type IDs, body bytes, serializer prefix, decode equality, byte-stable re-encode and frame checks | Run #8 |
| `tools/java-v1-golden/ProtocolV1GoldenFrameTest.java` | Four Java V1 frame decode/re-encode tests | 4/4 passed in Run #8 |
| `tools/java-v1-golden/AbstractTCInboundHandlerExceptionTest.java` | Eight selected Java TC exception/status fallback cases | 8/8 passed in Run #18 |
| `tools/java-v1-golden/SessionHolderRetryTimeoutRecoveryTest.java` | Three FILE/RAFT retry-timeout reload branch cases | 3/3 passed in Run #25 |

## 4. Phase 0.9 Java TC bootstrap harness

Harness: [tools/java-tc-integration](../../tools/java-tc-integration/README.md)

Workflow: [Phase 0.9 Java TC FILE Bootstrap](../../.github/workflows/phase-0-java-tc-file-smoke.yml)

Pinned Java source: `cba73e180fd8896a99df6415b859d275122feaa7`.

The harness:
- clones and checks out the pinned Java source;
- builds the server reactor with Maven;
- starts the server using an isolated FILE storage configuration;
- waits for the native TCP listener;
- supports stop and isolated storage reset.

**Execution status:** scripts and workflow are committed, but no successful Phase 0.9 bootstrap workflow result is recorded in this ledger yet. Update this section with the workflow run URL, tested Seata-Rust commit, Maven result, server process result and listener result when CI finishes.

**Evidence boundary:** even a passing bootstrap workflow proves only that the pinned Java TC process starts and accepts TCP connections. It does not prove RegisterTM/RegisterRM, Seata V1 frame acceptance by the live TC, transaction lifecycle, RM callbacks or persistence recovery.

## 5. Evidence ledger

| ID | Claim / gate | Evidence level | Status |
|---|---|---|---|
| SRC-001 | Java source contains V1 decoder/encoder and serializer | Verified in pinned source | Complete |
| SRC-002 | Go native codec prepends big-endian uint16 message type | Verified in pinned source | Complete |
| GO-001 | Pinned Go encoder reproduces all 29 payload vectors | Verified by test, Run #8 | Passed |
| GO-002 | Pinned Go serializer reproduces all 4 full frames | Verified by test, Run #8 | Passed |
| JAVA-001 | Java V1 decoder accepts and re-encodes the 4 fixtures byte-identically | Verified by test, Run #8 | Passed |
| TC-001 | Selected Java TC request exception mapping | Verified by test, Run #18 | Passed for covered cases |
| TC-002 | Java retry-timeout FILE/RAFT reload branches | Verified by test, Run #25 | Unit-level passed; durable recovery pending |
| ENV-001 | Pinned Java TC executable server build | CI runtime required | Pending Phase 0.9 workflow |
| ENV-002 | Java TC FILE startup and TCP listener | CI runtime required | Pending Phase 0.9 workflow |
| INT-001 | Live RegisterTM over native Seata V1 | Live TC integration required | Not implemented |
| INT-002 | Live RegisterRM over native Seata V1 | Live TC integration required | Not implemented |
| INT-003–010 | GlobalBegin, BranchRegister, Status, Commit/Rollback and RM phase-two callbacks | Live TC integration required | Not implemented |
| INT-011–014 | FILE restart, branch persistence and response-loss reconciliation | Live TC integration required | Not implemented |
| RUST-001 | Rust protocol implementation matches Java/Go | Rust implementation/test required | Not started |

## 6. Phase 0 exit decision

**Protocol codec/frame sub-gate: passed.** GO-001, GO-002 and JAVA-001 have runtime evidence.

**Selected Java TC unit-test sub-gates: passed.** TC-001 covered cases and TC-002 FILE/RAFT reload unit branches have test evidence.

**WP-00 / Phase 0 as a whole: still open.** Remaining gates include:
- Phase 0.9.1 Java TC FILE build/start smoke;
- live RegisterTM/RegisterRM and transaction lifecycle over native V1;
- RM callback handling;
- durable FILE restart and operator retry;
- remaining Java TC exception matrix cases;
- all-message Java serializer equivalence and eventual Rust codec conformance.

Do not claim full Seata-Rust V1 protocol conformance or production readiness from the current test set.

## 7. Reproduction commands

From the Seata-Rust repository root:

```bash
bash tools/go-golden-vectors/verify.sh
bash tools/java-v1-golden/verify.sh
bash tools/java-tc-integration/scripts/build-tc.sh
bash tools/java-tc-integration/scripts/smoke.sh
```

Prerequisites:
- Git and network access to GitHub;
- Go 1.23;
- JDK 17 and Maven;
- network access for Maven and Go dependencies.
