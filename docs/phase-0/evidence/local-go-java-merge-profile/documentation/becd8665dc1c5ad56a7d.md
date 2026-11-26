# Seata-Rust Phase 0 — Compatibility and Integration Validation Ledger

> **Current status (2026-11-12):** 29 frozen payload samples pass Go, Java and independent Rust test codecs. Selected live Java TC registration, lifecycle, callback response loss and normal FILE restart cases have separate local evidence below. Phase 0 remains open; production Rust components are not yet delivered.

Latest failure increment: Rust81/81 and Java target56/56 pass; seven supplemental cases are independently checked, with four byte-compatible pairs and **three OPEN Go/Java differences**. Diagnostic green is not an interoperability pass. [Known issues](known-go-v1-failure-divergences.md); [exact evidence](evidence/local-failure-variants-20261112/README.md).

Latest handler increment: Java target**135/135** (72 new handler +7 registration processor cases),91-module BUILD SUCCESS. The eight-handler exception/fallback/lock-conflict and auth-hook unit matrix is covered; actual network authentication and every-runtime-payload Go/Rust comparison remain open. JAVA-V1-REPORT-NULL-001 records an actual null-status response encoding failure. [Evidence](evidence/local-java-handler-matrix-20261112/README.md).

Latest runtime-output increment supersedes that pending comparison: fresh Java135 emits all79 events;78 serializable payloads pass Rust field/byte checks (full suite83/83 at that increment). Pinned Go78 native checks yield62 compatible,6 known length differences,1 known ordinal difference and9 byte-preserved opaque status19 cases (GO-V1-STATE-001). Null-report remains separate and unencodable. [Exact capture/negative controls](evidence/local-handler-runtime-20261112/README.md). Comparison is complete for the selected matrix; interoperability issues and live auth remain open.

Latest external-loss increment: live proxy drops one complete successful BranchRegister/Commit/Rollback response per case; no original mutation is replayed. Registration identity is learned from a validated nonce callback. End queries return Finished15 and retain uncertainty. Default and spaced-directory runs pass, with lock release and server callback processing independently checked. Rust93/93, Python5/5, static checks pass. [Evidence](evidence/local-phase09-response-loss/README.md).

Latest FILE retry-timeout diagnostic: two real persisted-record/full-JAR runs reproduce startup failure for16 and filtered session/branch plus missing lock for17. Begin1 positive control restores correctly. Initial Java combined140/140; final runner freshly rebuilds/runs5 focused process tests. Rust96/96. Recovery acceptance remains OPEN/failed, not green. [Exact evidence](evidence/local-file-retry-timeout/README.md).

Latest DB/Redis recovery increment: actual cached backend process restarts, fresh reader/original TC JAR, console retry and native RM checks pass all8 seeded-state cases. Original locks/branches and empty profiles are verified, repeats require exact missing-session proof, and observers are explicitly completed before physical store checks. Java selected3/3, Rust98/98, Python5/5. JAVA-METRICS-RETRY-001 remains open; natural deadlines, live RAFT and real compensation/durable replay remain unverified. [Evidence](evidence/local-backend-retry-timeout/README.md).

Latest authentication increment supersedes the earlier pending live-auth statements: three real TC profiles pass with the original JAR;16 TM/RM denials leave no FILE transaction, and a fresh authenticated RM re-registers both cached resources and receives two original branch callbacks. Focused Java7/7, final combined Java150/150 and Rust103/103 pass, including actual SPI selection and asynchronous query-only completion regressions. [Evidence](evidence/local-auth-registration/README.md). REG-003–006 pass in this fixture scope; production policy/TLS and OPEN JAVA-AUTH-EXTRADATA-001 remain separate.

Latest RAFT increment supersedes earlier unknown-live-RAFT statements: three fresh three-node cohorts complete six real leader transitions and three original snapshot reloads.16/17 retain original records/branches/locks and answer actual console/native queries, while four exact current-transition JFR events prove asynchronous reload failure. Begin controls work as documented. Java4, Rust106 and Python3 pass as diagnostic checks; REC003/004 safe recovery acceptance and JAVA-RAFT-RELOAD-001 remain open. [Evidence and other metadata/logging observations](evidence/local-raft-retry-timeout/README.md).

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
| `tools/go-golden-vectors/frames.json` | 6 full V1 frames: 4 begin cases plus RegisterTM and RegisterRM | Go runtime verified in Run #8 and Run #40 |
| `tools/go-golden-vectors/main.go` | Type IDs, body bytes, serializer prefix, decode equality, byte-stable re-encode and frame checks | Run #8 |
| `tools/java-v1-golden/ProtocolV1GoldenFrameTest.java` | Six Java V1 frame decode/re-encode tests, including TM/RM registration | 6/6 passed in Run #40 |
| `tools/java-v1-golden/AbstractTCInboundHandlerExceptionTest.java` | Eight selected Java TC exception/status fallback cases | 8/8 passed in Run #18 |
| `tools/java-v1-golden/SessionHolderRetryTimeoutRecoveryTest.java` | Three FILE/RAFT retry-timeout reload branch cases | 3/3 passed in Run #25 |
| `tools/java-v1-golden/PayloadGoldenCorpusTest.java` | 29 Go sample payloads: Java class/type, independent field semantics and identical re-encode | 29/29 passed locally 2026-11-12; target combined suite 46/46 |
| `tests/fixtures/protocol/v1/` | 29 raw serialized binary payloads with source/input/codec/hash metadata | Byte-equal to unchanged canonical JSON; generator 6 tests pass |
| `tools/java-tc-integration/rust-client/src/payload/` | Test-only typed codecs for 25 message types; all 29 independent semantic/byte assertions, malformed boundaries and single-byte mutations | 73/73 full Rust suite (39 new + 34 existing), fmt/Clippy exit 0; [evidence](evidence/local-rust-payloads-20261112/README.md) |
| `tools/java-v1-golden/PayloadOptionalityTest.java` | Nullable report status and TM/RM absent response versions | 3/3 fixed Java oracle; updated combined target 49/49, 91-module BUILD SUCCESS; [evidence](evidence/local-rust-payloads-20261112/README.md) |
| `tools/go-failure-variants/` + `FailureVariantCompatibilityTest.java` | Six remaining failure response families and fail-fast error ordinal comparison; original29 unchanged | Go native7/7, Java7/7, Rust8 new tests; combined Java56/Rust81; **4 compatible, 3 OPEN** |
| `tests/fixtures/protocol/v1/failure_variants/` | Seven separate Go/Java payload pairs, typed inputs, commits and hash metadata | 14 binaries checked, exporter5tests; generation itself is not certification |
| `TCInboundFailureMatrixTest.java` | All8 handlers' normal/3exception outcomes, status fallback edges, conflict codes, lockable=false and null-report diagnostic | 72/72 passed; manual independent Java wire layout for serializable outcomes; null report encoding remains OPEN |
| `RegistrationProcessorAuthTest.java` | TM/RM allow/deny/throw hooks and denied closed RM cleanup; actual V1 flag semantics | 7/7 passed; mocked bookkeeping, not live auth; combined target135/135 |
| `tools/java-handler-wires/` + `handler_matrix/` | Capture complete79 real case events, export78 payload/field pairs; actual Go/Rust comparison | Collector6tests, Rust83/83; Go62/6/1/9 diagnostic split; all source/report/fixture hashes verified |

### Run #40 — RegisterTM/RegisterRM V1 frames added

[GitHub Actions Run #40](https://github.com/easy-4-rust/seata-rust/actions/runs/42473731588) passed after adding complete RegisterTM and RegisterRM V1 frames.

- Go: 29 payload vectors and all 6 complete V1 frames passed.
- Java: 6 V1 frame Decoder → Encoder round trips passed, including RegisterTM and RegisterRM.
- The same Java Maven run executed 8 TC exception tests and 3 FILE/RAFT reload tests: **17 tests, 0 failures, 0 errors, 0 skipped**.
- The Java RM registration request decoder maps a zero-length `extraData` field to `null`, while TM's shared IdentifyRequest decoder maps it to an empty string. The test now asserts the Java behavior explicitly; byte re-encoding remains stable.

This verifies registration payload/frame compatibility against the pinned Go and Java codecs. It is not a live RegisterTM/RegisterRM exchange with a running TC.

### Run #42 — Registration V1 frames and all current Java unit overlays

[GitHub Actions Run #42](https://github.com/easy-4-rust/seata-rust/actions/runs/42474150028) passed on the registration contract/test commit.

- Go: 29 payload vectors and 6 complete V1 frames passed, including RegisterTM and RegisterRM.
- Java: 6 V1 frame round trips passed; 8 selected TC exception tests passed; 3 FILE/RAFT reload tests passed.
- Maven result: **17 tests, 0 failures, 0 errors, 0 skipped; BUILD SUCCESS**.
- Java's RegisterRM codec decodes zero-length extraData to null, unlike the TM identify-request decoder which produces an empty string. Both re-encode to the same zero-length wire field; the fixture asserts this difference.

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

**Execution status:** the harness has reached the Java ServerApplication startup path, but no successful end-to-end smoke result is recorded yet. Run #14 failed during Maven dependency resolution with a transient Maven Central HTTP 504 while resolving Kotlin compiler 2.2.20; the build script now retries Maven up to three times. The current retry-enabled bootstrap run is pending. Record its final run URL, Maven result, server process result and listener result before marking ENV-001/002 passed.

**Evidence boundary:** even a passing bootstrap workflow proves only that the pinned Java TC process starts and accepts TCP connections. It does not prove RegisterTM/RegisterRM, Seata V1 frame acceptance by the live TC, transaction lifecycle, RM callbacks or persistence recovery.

### Run #15 — Java TC FILE bootstrap passed

[GitHub Actions Run #15](https://github.com/easy-4-rust/seata-rust/actions/runs/42474316751) passed on 2026-11-08.

- Pinned Java Seata source checked out at `cba73e180fd8896a99df6415b859d275122feaa7`.
- Maven `release-seata-jar` build succeeded and produced `server/target/seata-server.jar`.
- Java ServerApplication started with the isolated FILE configuration.
- TCP listener accepted connections at `127.0.0.1:18091`.
- Smoke script stopped the TC process successfully.

This is TCP bootstrap evidence only. Run #15 did not exchange Seata V1 messages. The independent Rust registration client was added after Run #15; live RegisterTM/RegisterRM will be verified by the subsequent workflow.

### Run #23 — Live Rust RegisterTM/RegisterRM against Java TC

[GitHub Actions Run #23](https://github.com/easy-4-rust/seata-rust/actions/runs/42475353541) passed on 2026-11-08.

- Pinned Java Seata TC built and started with isolated FILE storage.
- Rust standalone client compiled and passed 2 unit tests.
- Over a live native V1 TCP connection, Java returned RegisterTM `identified=true`.
- Over a second live native V1 TCP connection, Java returned RegisterRM `identified=true` for resource ID `mysql://orders`.
- Both responses reported server version `2.8.0-SNAPSHOT`.
- Java TC shut down cleanly after the test.

**Boundary:** this run used `enable-check-auth=false`; it verifies the unauthenticated registration path, not authentication. It also does not yet test GlobalBegin, BranchRegister, phase-two callbacks or recovery. The client is being aligned to Seata-Go's actual `constant.SeataVersion = "1.1.0"` and the FILE data path has been corrected to the resettable data directory; those adjustments require a subsequent CI confirmation.

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
| REC-FILE-DIAG-001–002 | Persisted retry-timeout FILE records and unchanged full TC startup | Two local3-case runs2026-11-12; Begin control, original-XID and lock checks | Diagnostic reproduced:16 startup failure,17 session/branch/lock restoration absent; REC-TIMEOUT-001–002 recovery acceptance remains open; [evidence](evidence/local-file-retry-timeout/README.md) |
| REC-RAFT-DIAG-003–004 | Real leader changes, quorum replication, snapshots and async reload | Local2026-11-12, three cohorts, six elections,39 replica state sets,18 native queries and four original reload exception events | Diagnostic complete;16/17 retained/queryable but reload throws. Safe recovery acceptance open; [evidence](evidence/local-raft-retry-timeout/README.md) |
| REC-DB-REDIS-005–007 /010 | Store restart, original records/locks, console manual retry, empty profiles and safe repeat rejection | Local2026-11-12 final8 cases, original JAR cohort, actual Docker PID changes, native original callbacks | Scoped pass;008–009 durable replay/compensation remain partial, metrics null-endTime issue OPEN; [evidence](evidence/local-backend-retry-timeout/README.md) |
| JAVA-COMBINED-001 | Exact current Java harness on a fresh pinned snapshot | Local 2026-11-12, 91-module reactor, selected 17 target tests | Passed; executed/harness hashes equal; [evidence](evidence/local-java-exact-20261112/README.md) |
| JAVA-PAYLOAD-001 | All 29 current Go payload samples cross-decode and re-encode in Java V1 | Local 2026-11-12, selected target suite 46 tests | Passed, including independent field semantics; [evidence](evidence/local-java-payloads-20261112/README.md); Rust full-corpus conformance remains open |
| ENV-001 | Pinned Java TC executable server build | Verified by test, Run #15 | Passed |
| ENV-002 | Java TC FILE startup and TCP listener | Verified by test, Run #15 | Passed |
| INT-001 | Live RegisterTM over native Seata V1 | Verified by test, Run #23; auth disabled | Passed for unauthenticated path |
| INT-002 | Live RegisterRM over native Seata V1 | Verified by test, Run #23; auth disabled | Passed for unauthenticated path |
| REG-003–006 | Live auth-hook denials and cached-resource reconnect/routing | Local2026-11-12, three fresh TC profiles,16 denied-channel stimuli, two original callbacks | Passed for test SPI/simulated RM; production authentication remains open; [evidence](evidence/local-auth-registration/README.md) |
| INT-003 / INT-005 | GlobalBegin and GlobalStatus | Live pinned FILE TC, local 2026-11-11 | Passed for two empty transactions; [evidence](phase09-local-validation-results.md) |
| INT-EMPTY-001 | No-branch GlobalCommit / GlobalRollback | Live pinned FILE TC, local 2026-11-11 | Passed; responseStatus 9 / 11 followed by Finished=15 |
| INT-004 / INT-006–009 | BranchRegister, Commit/Rollback and RM phase-two callbacks | Live pinned FILE TC, local 2026-11-12 | Passed for simulated AT resource; [evidence](evidence/local-phase09-branches/README.md) |
| INT-010 | Duplicate phase-two callbacks and idempotency | Live pinned FILE TC, local 2026-11-12 | Partial: one withheld response followed by TC redelivery; successful simulated action applied once in-process; [evidence](evidence/local-phase09-callback-loss/README.md) |
| INT-011–012 | FILE restart after Begin/BranchRegister | Live pinned FILE TC, local 2026-11-12 | Passed for normal records: original identities and restored/released lock; [evidence](evidence/local-phase09-restart/README.md) |
| INT-013–014 | BranchRegister/TM response-loss reconciliation | Live pinned FILE TC + external proxy, local 2026-11-12 | Passed for one same-connection loss each: no resend; registration learned by validated callback; Finished-only end remains unknown; [evidence](evidence/local-phase09-response-loss/README.md) |
| CONTROL-001 | Heartbeat/merge frame generation, typed decode and original Go Getty capability | Local2026-11-12 fresh Java163/Rust114/Python5; nine control frames; [evidence](evidence/local-control-envelopes/README.md) | Three heartbeats byte-compatible; six merges Java/Rust only, original Go59/60 codecs absent (GO-V1-MERGE-001 OPEN); test-only Rust |
| RUST-001 | Rust protocol implementation matches Java/Go | [Independent test-codec corpus](evidence/local-rust-payloads-20261112/README.md) | 29 sample payloads verified; formal production crate and all variants remain open |

| INT-CLIENT-001 | Rust standalone TM/RM registration client compiles and validates response frame | Run #23; local 2026-11-11 has 13 tests plus live TC registration/lifecycle | Passed for covered paths; TM/RM version alignment reverified locally |

## 6. Phase 0 exit decision

**Original sample codec/frame sub-gate: passed.** GO-001, GO-002 and JAVA-001 have runtime evidence for their frozen inputs; this does not close the later failed-response/merge differences.

2026-11-12 revalidation: the exact current 6-frame + 8-exception + 3-reload overlays passed locally on a fresh pinned source snapshot. Existing Run #42 Java/Go jobs were directly verified completed/success. This does not claim a fresh current-head workflow run or an earlier failed-run root cause.

**Selected Java TC unit-test sub-gates: passed.** TC-001 covered cases and TC-002 FILE/RAFT reload unit branches have test evidence.

**Java TC bootstrap sub-gate: passed in Run #15.** The self-contained Java TC builds, starts, binds TCP 18091 and shuts down with isolated FILE storage.

**WP-00 / Phase 0 as a whole: still open.** Remaining gates include:
- arbitrary duplicate/reconnect and broader loss behavior (one withheld RM callback response/redelivery and three external mutation-response loss cases passed with process-local simulated state; Finished-only end outcomes remain unknown);
- database AT/Undo Log compensation and long-lived RM behavior;
- broader retry-timeout/operator recovery: FILE16 startup failure and FILE17 filtering remain external issues; DB/Redis eight seeded cases pass with simulated RM; real RAFT now proves retained16/17 plus failed async reload. Safe RAFT recovery/operator retry, natural deadlines and durable compensation/replay remain open; metrics and metadata/logging errors are separately recorded;
- production authentication/TLS and OPEN JAVA-V1-REPORT-NULL-001 / JAVA-AUTH-EXTRADATA-001. Bounded live auth-hook/reconnect, handler/processor unit branches and all78 serialized selected-matrix comparisons are now covered;
- GO-V1-STATE-001: raw Java status19 is preserved but business-state semantics are not established by the pinned Go declarations;
- resolve GO-V1-FAIL-001–003 (type14/22 failed text lengths and fail-fast ordinal), wider message variants and production Rust codec conformance. Heartbeat/merge now has nine-case evidence: three heartbeat frames match all three runtimes; six merges expose GO-V1-MERGE-001 (missing Go59/60 codecs), not interoperability. Supplemental failed BranchReport/GlobalLockQuery samples now expose an actual mismatch; they are not unexecuted or interoperable. The original29 sample sub-gate stays passed.

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

**Control-envelope update:** nine new Java-generated frame samples are retained separately with per-case certification. Java163 selected tests, Rust114 probe tests, Python5 capture/provenance checks, Clippy and formatting passed. Original Go Getty matches three heartbeats and lacks59/60 codecs for six merges; GO-V1-MERGE-001 remains OPEN. All18 Java overlay inputs were frozen before Maven; executed snapshots are separate, including explicit legacy formatting differences. Updated wrappers passed separate syntax checks only; no fresh Linux/current-head CI run is claimed. [Evidence](evidence/local-control-envelopes/README.md); [WP-00 remaining gates](wp00-freeze-readiness-audit.md).

**Explicit length-patch candidate update:** local `run_20261112_181635_276c0f53` passes Java187/14 suites, Rust115 and static checks. The unchanged original Go fails exactly21 named failed-response cases and passes three named success controls; the separate patched cohort passes all24 plus80 top-level codec/bytes tests and the original29/six-frame corpus. Python6 capture/provenance tests pass; existing control Python5 separately pass. Original714-file Go closure and2122 Java production files remain unchanged. GO-V1-FAIL-001/002 have a bounded candidate remedy, not an upstream resolution. [Evidence and limits](evidence/local-go-java-length-patch/README.md). The [staged-freeze proposal](wp00-staged-freeze-proposal.md) is pending confirmation; effective WP dependencies and full acceptance remain unchanged.

**Exception-profile candidate update:** `run_20261112_184713_f94f053f` and its linked parent pass fresh Java437/15 suites, Rust116, Go104 top-level codec/bytes/errors tests and static checks. Same Java250-case matrix yields original152/98, length-only190/60, explicit profile250/0; exact identities and250 pointer/caller-preservation checks are verified. All seven earlier Java failure semantics, original14 response payloads,15 unsupported-sample rejections and24 length-wire cases pass in the explicit profile; native29/six-frame defaults regress cleanly. New Python6, length6 and control5 separately pass. Consumed seven-case oracle and both Java fixture resources are now frozen/audited. Original baseline remains incompatible;0002 is opt-in and not transport-integrated. [Evidence and limits](evidence/local-go-java-exception-profile/README.md). WP-00 staged-freeze approval is still pending; no effective acceptance change or production-crate completion is claimed.

**Merged-payload candidate update:** final `run_20261112_193633_d41d73bc` passes Java475/16 suites, Rust117, Go110 top-level tests and new Python7.38 new plus six original merges pass field/raw-byte/pointer/caller/Java-semantic checks;29 ordinary payloads and250 nested exception meanings are covered. Signed timeouts, explicit absent-version metadata and nullable report status are added; Java default-version re-encoding differences are preserved honestly. Final fresh control env and complete inherited candidate source audit resolve two P2 driver findings. Existing exception6/length6/control5 Python checks separately pass. [Evidence](evidence/local-go-java-merge-profile/README.md). Required Getty integration, original/default compatibility, production Rust and WP-00 exit remain open; staged-freeze approval has not arrived.
