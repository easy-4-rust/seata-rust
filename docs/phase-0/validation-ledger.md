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

## Getty 显式帧候选增量（2026-11-12）

0004在独立副本显式注入实际RpcPackageHandler，验证分片、连续帧、错误传播、有序可空头及长度边界。完整新鲜Java475/Rust117/Go181通过，新Python6和继承merge7/exception6/length6/control5通过。326帧中9帧由Java实际编码，317帧由工具封装Java认证载荷，不扩大Java帧认证数。

未修改的Java TC通过注册、Begin/Status/Begin合并请求、两个原始XID的Begin查询/回滚/Finished查询，8请求/8响应、零事务重发；端口和自有进程已清理。实际handler由自定义TCP探针调用，尚未验证dubbo-getty会话循环和原SDK的适配选择/完整业务分发。[证据](evidence/local-go-java-getty-profile/README.md)，[后续运行时缺口](go-getty-runtime-gap-audit.md)。原版/default兼容、生产Rust、RM/Undo和WP-00仍开放；分阶段冻结提案尚未获确认。

## SDK 会话与关联候选增量（2026-11-12）

0005显式配置接入原SDK InitGetty/SessionManager/dubbo-getty loop/listener/processor/future；新鲜Java475/Rust117/Go235（七包、222继承+13新增）和Python9通过，继承getty6/merge7/exception6/length6/control5另行通过，并发批响应重复20次通过。原掉响应、部分完成/越界、错误信封/类型、并发混合批结果及配置冲突均有失败/修复证据。

真实SDK17请求/17实际TC响应、两连接注册/心跳、11次重复并合并发送、5次晚到原响应和6种注入得到5次类型化未知结果；两个原XID回滚后Finished，零事务重发，受检future清理。原Getty EOF不自动重连已有独立失败与带owner/registry校验的候选修复，默认路径未自动启用。[证据](evidence/local-go-java-sdk-profile/README.md)。

新发现[JAVA-V1-PREFACE-001](java-v1-preface-fragmentation.md)保持OPEN：固定TC拒绝首个magic字节单独到达，实际负向连接/源码已归档。SDK场景只强制TC→SDK分片，不能据此声称任意双向分片兼容。原版本默认缺口、完整客户端初始化/自动聚合、在途故障恢复、RM/真实Undo、生产Rust及WP-00仍未完成；待确认的冻结提案不自动生效。

## Java 前导分片候选增量（2026-11-12）

原版已有569项回归通过；新48项在原版29失败/19控制通过，候选完整617项/27套、Rust117/Go235/Python7及静态检查通过。独立Java补丁保留原ProtocolDetector，增加可选等待、独立索引与可读起点判断，Seata等齐3字节版本前缀。

同一新鲜构建的10个class独立overlay启动原JAR，5主类实际来源核对；原JAR单字节拒绝再次复现，候选实际SDK双向强制分片17请求/17响应、双连接心跳、重复/晚到/未知结果及两个原XID回滚通过，零事务重发。HTTP仅认证404/SETTINGS。721项输入含554固定夹具，另有24个Python输入导入前冻结。

[候选证据](evidence/local-java-preface-candidate/README.md)、[问题及范围](java-v1-preface-fragmentation.md)、[实施计划](java-preface-candidate-plan.md)。原制品仍OPEN；候选未合入发布，V0继承问题/V2专门分片认证、完整客户端、真实RM/Undo、生产Rust、WP-00及Phase1–7仍未完成，冻结提案仍待确认。

## SDK 在途结果候选增量（2026-11-12）

对应[在途验收计划](go-java-v1-inflight-plan.md)及已接受ADR-0008。0006只在显式同步路径区分未发送与结果未知，绑定真实会话/确切future，保留TC明确响应；完整Go254（235继承+19新增）、Rust117/Python5及静态检查通过，Java617明确继承已认证父证据，本轮未重跑。

原0005同一19项行为对照、处理器暂停/ID复用、binder查找窗口和并发代理分片均保留失败证据。统一短锁及入站对象快照修复归属竞态，合法TC→RM同ID请求不误丢弃；旧子项批次新增成功/整批未知/新所有者保护控制。代理整帧分片按连接/方向加锁。

最终实际SDK四连接均注册，19请求/19实际TC响应，Begin4/Commit1/Rollback3、查询7；三次响应丢失断连和一次自然20秒超时均返回可识别UnknownOutcome，晚到原响应不复活等待。Commit状态实际8→15，只查询不重发事务，四原XID最终核对/清理。[归档证据](evidence/local-go-java-inflight/README.md)。

原版/default及前五补丁、旧副本与原Java JAR保持不变。未知Begin的XID来自测试TC观察者，不代表通用Begin幂等恢复；完整业务/异步客户端、自动聚合发送策略、真实RM/Undo、TLS、生产Rust、完整WP-00与Phase1–7仍未完成，分阶段冻结提案仍待确认。下一步继续完整客户端初始化/资源注册与自动聚合，以及FILE/RAFT恢复和真实补偿缺口，不重复把选定同步链路计为全部交付。

## 完整Go SDK与真实MySQL AT增量（2026-11-13）

按[真实AT计划](go-java-v1-real-at-plan.md)完成三场景新鲜验证：原client.InitPath/TM/RM/SQL代理正常提交和补偿；阶段一后断连及真实资源重注册，重复回滚生成status1防悬挂记录且业务不再改变；原SDK进程退出、MySQL重启后原XID/BranchId/完整Undo字节不变，新PID补偿原事务且不发第二Begin。TC终态查询和独立业务/Undo SQL分别核对。[完整证据](evidence/local-go-java-real-at/README.md)。

Python16通过，759输入导入前冻结，Java覆盖类固定清单/10class字节链及每场景前后/实际加载来源核对。原Go714成员及既有24候选/13辅助未改，Go254/Rust117/Java617明确继承已认证父证据，本轮未重跑其单元测试。审查发现的恢复代理/关联、空SQL快照和class哈希缺口经真实负向测试后修复；期望的prepare断连单独分类，其他错误不放过。

这些是参考Go真实资源证据，不是生产Rust。正常Begin/Registered持久重放不能替代RetryTimeout16/17的008–009完整验收；全SQL/故障原子性矩阵、混合语言共享资源、生产认证及完整WP-00/Phase1–7仍开放。新记录[GO-AT-RESOURCE-ID-001](go-at-resource-identity-divergence.md)：原Go标识含DSN凭据且命名不同，不能据单Go RM成功推断Java/Go/Rust共享锁安全。后续推进该边界及真实重试超时恢复，既有冻结提案仍待确认。


## 真实AT自然重试超时与持久重放增量（2026-11-13）

依据[真实AT超时计划](go-java-v1-real-at-timeout-plan.md)完成新的MySQL DB存储TC联合运行：完整Go SDK只创建一次原事务和真实SQL/Undo分支，代理阻断实际回滚指令，原调度器自然写入RollbackRetryTimeout17；原TC/SDK进程退出及MySQL重启后，原XID/BranchId/资源/锁与完整Undo字节保留。实际控制台人工重试由真实RM补偿余额900→1000并删除Undo，成功响应被代理丢弃，TC仍保持17及原分支/锁；再次TC/SDK/MySQL重启后再人工重试原分支，余额保持1000，TC记录/分支/锁清理，Go RM写入同原身份status1防悬挂Undo。[正式证据](evidence/local-go-java-real-at-timeout/README.md)。

三个不同TC和完整SDK进程、两次数据库重启、实际注册/分支/回调连接与请求ID、五阶段SQL/控制台及自然17日志分别核对。首次HTTP500必须与原分支响应超时对应，最终成功不只依赖HTTP200。本轮新执行Python25通过，包含保持帧哈希/序号合法的原始证据负向用例；Go254/Rust117/Java617明确继承父证据，本轮未重跑。770项输入导入前冻结；固定JDBC8.0.33在每阶段前后校验字节与实际class来源，原Java JAR与Go候选副本未改。失败收尾尝试及修复后完整重跑均保留，正式运行容器/进程已停止、端口已释放。

REC-TIMEOUT-009的真实MySQL AT补偿/丢响应/持久重放取得该链路证据，仍非所有SQL、并发崩溃和存储组合验收。REC-TIMEOUT-008/自然16仍开放：固定Java纯AT通常走AsyncCommitting，不能靠改状态或短时观察声称自然16。FILE/RAFT缺陷、资源标识差异、生产认证、正式Rust、完整WP00和Phase1–7仍未完成；既有分阶段冻结提案仍待确认。下一步补真实提交重试及剩余恢复边界，不把选定参考链路当成生产交付。


## 真实AT＋TCC自然提交超时与持久重放增量（2026-11-13）

按[真实混合提交计划](go-java-v1-real-commit-timeout-plan.md)完成新鲜运行：完整Go SDK建立原AT/TCC两分支，真实AT业务余额1000→900并生成Undo，TCC业务真实预留10。仅阻断真实TC Commit指令，让原调度器自然持久化CommitRetryTimeout16。TC/SDK/MySQL重启后保留原XID、两BranchId、资源、AT Undo完整字节和TCC预留。人工重试时原AT实际清理Undo并答复，原TCC本地确认10后成功响应丢失，TC维持16且只剩原TCC分支；再次TC/SDK/MySQL重启后人工重试原TCC，确认仍10、效果计数仍1，最终TC global/branch/lock和Undo均空，AT余额保持900。[正式归档](evidence/local-go-java-real-commit-timeout/README.md)。

原TM仅一次Begin/Commit、无Rollback或目标重发。首次operator为HTTP200/body code500，须匹配原TCC等待响应超时及AT提交成功日志，不能当成完全成功；最终HTTP200/code200同时经过帧、独立SQL及控制台验证。每个callback与原分支类型/资源/连接/请求ID及唯一成功回复绑定。审查发现的重复TCC成功替代AT回复、错误envelope、SQL冻结遗漏已修复；合法不同V1 header负向用例有修前RED和修后GREEN。

本轮Python27新执行通过；Go254/Rust117/Java617明确继承，未重跑。783输入导入前冻结，含新增schema.sql；固定JDBC8.0.33/原JAR、每轮实际Driver类来源及10个覆盖类字节/5主类来源前后核对。自有容器/进程已停止，端口释放。原TC重试指标订阅者的null endTime异常再次出现，JAVA-METRICS-RETRY-001保持OPEN，见归档事件；未将业务恢复通过冒充指标已修复。

这是REC-TIMEOUT-008选定AT＋TCC真实提交恢复与持久业务幂等证据，与此前009真实回滚链路互补。TCC幂等来自本次业务的本地事务/唯一身份/行锁，不表示SDK自动保障任意业务；纯AT AsyncCommitting也不是本次自然16触发源。全SQL/并发崩溃矩阵、完整TCC Fence、FILE/RAFT安全恢复、混合语言资源标识、生产认证、正式Rust、完整WP00及Phase1–7仍未完成。既有分阶段冻结提案仍待确认。下一步处理剩余存储恢复和资源兼容边界，保持全部计划范围。


## Java重试超时保留候选与真实FILE重启增量（2026-11-13）

按[候选恢复计划](java-retry-timeout-retention-candidate-plan.md)形成独立Java0002补丁：FileSessionManager不再过滤17，SessionHolder显式保留16/17且不激活、不改成自动重试；16不重新获取提交锁，17按原两个解锁开关决定保留回滚锁。DefaultCoordinator两个既有超时入口和恢复共用原启动快照，避免热变更provider产生生命周期分叉。未修改原Java源码/JAR、旧前导补丁或正式Rust策略。[完整证据](evidence/local-java-retry-timeout-candidate/README.md)。

最终Maven72项新鲜通过（37目标＋35受影响原测试），Python12项通过。相同编译测试字节对原JAR有23失败/14控制，对实际部署覆盖类37全过；独立JVM曾复现动态配置读取12失败/25控制，共享快照后通过。真实FILE六情形、12个完整TC进程：Begin控制保留锁；16成功启动并保留原会话/分支、提交锁释放；17保留原身份并遵守两flags四种组合。控制台与仅101/15/21原生查询分别核对，无Begin/End或RM资源注册。原2122生产文件仅三处候选差异，29仓库输入导入前冻结、两外部元数据及340classpath/2800实际依赖绑定，19覆盖类字节/三个候选实际来源核对。自有TC停止、28091释放。

旧夹具引导、缺extraData、误判类加载和编译元数据缺口等失败/较弱运行保留，不计正式通过。原未打补丁FILE16/17问题仍存在；候选只取得本次seeded真实FILE及单元范围证据，不冒称自然超时/真实RM补偿。RAFT分支单元覆盖不能代替实际选举/快照；原未观察CompletableFuture的失败安全性仍需专门验证。指标、RAFT元数据/日志兼容、混合语言资源标识、正式Rust、完整WP00与Phase1–7仍开放；分阶段冻结提案仍待确认。后续继续候选的真实RAFT重载与剩余资源恢复，不降低原验收。


## Java RAFT 候选真实选举与恢复完成观察增量（2026-11-13）

按[观察候选计划](java-raft-reload-observation-candidate-plan.md)新增独立0003补丁，组合既有0001/0002；原Java源码/JAR、旧补丁和正式Rust策略保持原样。实际SessionHolder.reload异步成功才记录捕获的任期，单调更新避免旧任务晚完成覆盖新成功；失败记录分组/任期/异常。标记仅供观察，不改变isLeader，不代表服务就绪或失败领导隔离。[完整证据](evidence/local-java-raft-reload-candidate/README.md)。

真实三节点覆盖Begin1对照、CommitRetryTimeout16及默认双解锁开关关闭的RollbackRetryTimeout17：6次领导切换、3次快照重载、12个独立TC JVM。16保留原身份/分支/状态且清理残锁；17保留原记录和锁，不转自动重试。18组当前领导确认核对Seata/JRaft实际前后任期及领导身份、第二次取样、复制位置和原记录。18次只读101/15/21查询的108原帧重新解码并核对完整请求；48条实际主类来源及6份JFR原件核验。

本轮新执行Java52项（8目标＋44上游RAFT原回归）、Python18项通过。同一目标二进制对原JAR为6失败/2控制，对实际运行覆盖类8通过；44项以正确Spring环境、实际覆盖类和编译依赖前后绑定执行。轻量环境回归失败及早期较弱运行保留作诊断，不计最终通过。53输入导入前冻结，父清单固定认证19类，340classpath/2800文件继承并核对；21覆盖类和实际JFR reader等生成类前后守卫。最终完整性失败先转失败再写结果。自有进程停止、9端口释放。

本候选取得seeded真实RAFT选举/快照恢复及异步成功观察证据；不是自然超时、真实RM补偿、额外解锁配置实测或失败领导隔离验收。原版FILE/RAFT、元数据/日志、指标、混合资源标识、正式Rust、完整WP00及Phase1–7仍未完成；分阶段冻结提案仍待确认。下一步验证自然超时后的FILE业务恢复与其余存储/资源边界，继续全部计划。


## FILE自然17与真实AT持久补偿增量（2026-11-13）

依据[FILE业务恢复计划](go-java-v1-file-at-timeout-plan.md)，完成独立19类Java候选的真实FILE链路：完整固定Go SDK只创建一次原事务/SQL/Undo并发起一次Rollback；代理阻断8个真实回滚请求，原调度器自然进入17。实际FILE journal经停止及新TC/SDK/MySQL重启后，原XID、BranchId、资源、回滚锁与完整Undo字节保留。人工操作由真实RM补偿900→1000并删除Undo，真实成功响应丢失后TC仍17及原分支/锁；再次停止重启后人工重放原分支，业务仍1000，原TC会话/分支/锁移除，Go持久化同原身份status1防悬挂Undo。[正式证据](evidence/local-go-java-file-at-timeout/README.md)。

MySQL仅accounts/undo_log，TC实际session/lock均FILE。3TC/3SDK与两次MySQL重启，日志/配置/PID/类来源及停机journal→下一轮启动原字节分别绑定；原journal在丢成功响应阶段也未变。五阶段独立SQL、控制台、只读原生5查询/30帧及SDK注册/回调/请求ID/方向/外层类型核对；原Begin回复及Rollback目标XID均绑定。首次HTTP500对应原分支响应超时，最终HTTP200/code200还须业务与TC清理证据。

本轮Python44项新执行通过（13归一化、8FILE、23真实证据破坏控制），含先前7/9/1项行为RED的修复。803输入导入前冻结，Go cohort及原JAR、SDK binary、配置、19覆盖类字节前后一致，24条实际主类来源核验；完整性检查先于成功写入。Go254/Rust117/Java617明确继承未重跑。首轮较弱校验运行保留诊断；自有TC/SDK/代理和容器停止，3端口释放。

REC-TIMEOUT-009取得本候选选定FILE自然17与真实业务持久重放证据；其他SQL/并发/崩溃、FILE自然16、RAFT真实业务、额外配置和生产策略继续按原范围验证。原版FILE/RAFT缺陷、指标/元数据/日志、混合资源标识、正式Rust、完整WP00及Phase1–7仍未完成，分阶段冻结提案仍待确认。下一步继续FILE自然提交超时和其余运行时边界，保持全部计划目标。


## FILE自然16与真实AT/TCC持久提交增量（2026-11-13）

依据[FILE混合提交计划](go-java-v1-file-commit-timeout-plan.md)完成独立19类Java候选的真实自然16链路。完整Go SDK只发一次Begin/Commit、零Rollback；真实AT本地SQL/Undo与TCC预留后，代理阻断9个原Commit请求，原调度器自然进入16。TC/SDK及MySQL重启后原XID、两分支、AT完整Undo、TCC预留保留且提交锁可用。实际人工重试中原TCC确认10后成功回复丢失，原AT真实清理Undo并答复；TC仍16且仅原TCC。再次重启并人工重放原TCC后，余额仍900、Undo空、确认仍10且效果计数仍1，原TC会话/分支清理。[正式证据](evidence/local-go-java-file-commit-timeout/README.md)。

MySQL仅四个业务/Undo/TCC表，TC实际session/lock为同步FILE。3TC/3SDK及两次MySQL重启，两次启动journal分别精确等于前次停机字节；部分提交移除AT与最终清理后journal正常追加。原13报告、成功14确认、保留分支状态/应用上下文和实际Commit回调逐项绑定，确认必须先于首次回调；原Begin/Commit目标、注册、方向、外层类型、连接/请求ID、真实丢帧和operator顺序均核对。五阶段独立SQL/控制台/5原生查询30帧，以及24条实际主类来源分别重算。

本轮Python57项新执行通过（13归一化、8FILE、36真实证据破坏控制），包含先前7/3/2项行为RED的修复。819输入导入前冻结，父783及共享代码/候选类认证；Go cohort、原JAR/SDK binary/配置/19类字节前后匹配，完整性检查先于成功写入。Go254/Rust117/Java617明确继承未重跑。早期两次较弱门禁运行保留诊断；自有资源停止，3端口释放。

JAVA-METRICS-RETRY-001本轮再次出现2条原指标订阅者null endTime异常，独立归档并保持OPEN，未将业务恢复通过冒充指标修复。REC-TIMEOUT-008取得本候选选定FILE自然16与持久业务幂等证据；完整TCCFence、RAFT真实业务、其他SQL/配置/并发/崩溃矩阵仍单独验收。原参考缺陷、元数据/日志、资源标识、生产认证、正式Rust、完整WP00及Phase1–7仍未完成，分阶段冻结提案仍待确认。下一步处理已复现指标异常与其他剩余边界，继续全部计划。


## Java指标生命周期候选与真实FILE16增量（2026-11-13）

依据[指标候选计划](java-metrics-lifecycle-candidate-plan.md)新增独立0004补丁：producer的doing事件有意保留null endTime，完成状态消费者须先区分未完成事件，避免重入16/17时误扣active、伪增失败并抛NPE。Begin/三Retrying保留原规则；有效done指标名称/标签/计数不改，缺begin/倒序/溢出仅跳Timer，不补零或当前时间。原JAR、producer及事件未修改。[正式证据](evidence/local-java-metrics-lifecycle-candidate/README.md)。

同一实际CompactRegistry测试字节对原JAR为47失败/30控制，候选77项通过（65目标＋12固定pin原回归）；4源先冻结再重编，测试/候选class与实际部署字节一致，278classpath/1950文件前后绑定。新执行Python72项：父业务57重跑、指标破坏15；异步完成标记曾2失败/13控制，修后全绿。Go254/Rust117/Java617完整父范围明确继承，不与77直接相加。

真实FILE16混合业务继续通过3TC/3SDK与两次journal/MySQL恢复、真实AT清Undo、TCC成功丢回复和持久重放。每进程SDK前基线独立全0；初始自然超时failed/timeout/timer各1且5486ms；两个恢复进程实际null-end doing被精确DEBUG标记确认后采样，重入不增完成计数、active保持0；最终真实AfterCommitted counter/summary/timer各1，实际3ms。8快照/8回执、原总线事件、30类来源及原SQL/控制台/5查询30原帧分别核对。只读get/count/total不measure/clear；Exporter仅绑独立端口，未scrape，不据此验收Exporter输出。

834仓库输入导入前冻结、40外部输入（含源码快照/类/编译参数/日志/D冻结脚本）及运行依赖完整性通过后才写成功。MySQL临时套接字误ready的失败及缺显式顺序屏障的早期运行保留诊断；正式采用TCP就绪和“同TC/原transactionId跳过标记→采样”门禁。自有资源停止，五端口释放。

JAVA-METRICS-RETRY-001原参考仍OPEN，0004在本候选FILE16事件生命周期范围取得通过证据；不等于全部重启计数重建/去重、Exporter、真实FILE17/RAFT指标或完整生产验收。其他SQL/配置/并发/崩溃、资源标识、认证、正式Rust、完整WP00与Phase1–7继续开放，分阶段冻结提案仍待确认。后续继续真实回滚指标及其余运行时门禁，保持全部计划目标。


## Java指标候选与真实FILE17回滚增量（2026-11-13）

依[回滚指标计划](java-metrics-rollback-candidate-plan.md)，[正式运行](evidence/local-java-metrics-rollback-candidate/README.md)为run_20261113_045658_cc75268a。实际3TC/3SDK、两次MySQL与原FILE journal恢复、实际补偿成功丢回复和同分支重放通过。初始自然17 failed/timeout/timer各1=5485ms，两个恢复进程nullable17 doing的实际处理标记先于采样；最终AfterRollbacked各1=3ms，active/failed仍0。8非重置快照、8回执、30类来源及5原生查询30帧、原RPC/SQL/Undo/控制台与两个完全相同恢复输入分别推导核对。

新执行Python71（44业务、12阶段样例、15实际副本破坏）；阶段旧校验RED3失败/9控制，修后全绿。Java77继承未重跑，本轮重新编译生产class与其认证字节相同；Go254/Rust117/Java617完整父范围明确继承。854仓库与40外部输入、278classpath/1950文件、原JAR/SDK/配置/Driver/候选类结束核对后成功，自有资源停止、五端口释放。

JAVA-METRICS-RETRY-001原参考OPEN；0004的FILE自然16/17分开运行生命周期通过，不等于并发组合、重启指标重建/去重、Exporter或RAFT全矩阵。REC-TIMEOUT-009本候选真实业务范围补充验证，正式Rust与WP00其余门槛不变，冻结提案仍待确认。


## 真实RAFT自然17与持久AT恢复增量（2026-11-13）

[计划](go-java-v1-raft-at-timeout-plan.md)对应[主运行052816](evidence/local-go-java-raft-at-timeout/README.md)。原TM一次Begin/Rollback，自然17；实际5TC JVM/3Go SDK、两次可见领导更替（采样1→2→4）、两次MySQL/SDK重启、真实补偿至1000丢原成功回复与同分支持久重放，最终三副本会话/分支/锁清理。六snapshot响应/文件、原protobuf index/term、两次原盘加载、两份JFR重读与停止窗口闭合，所有业务路由/恢复/五阶段观察绑定当前领导前后同PID/peer/term；20关键类来源及原RPC/独立SQL/控制台/五原生查询30帧分别核对。

运行65 Python＋独立归档停止身份4=69新执行。原集群RED15失败/2控制，follower RED4/17，输出RED3，完整证据旧校验RED8/1，停止身份RED3/1，当前全绿。最终门禁额外验证stop.before/identity/process等于前任确认leader，代码/测试/结果3输入冻结；不改本次runtime epoch，不以runtime succeeded单独认证。923运行输入、341classpath/2821依赖、20生产class/1控制、原JAR/SDK/Driver/配置均核对；Go254/Rust117/Java RAFT52继承未重跑。最终资源停止，11端口释放。

原参考JAVA-RAFT-RELOAD-001及metadata/logging仍OPEN，候选选定真实自然17恢复通过；18次metadata字符串命中不是独立事件数。未覆盖RAFT指标、自然16、生产认证及全分区/并发/损坏/崩溃、正式Rust、完整WP00和Phase1–7；冻结提案待确认。


## 真实RAFT自然16混合提交增量（2026-11-13）

[计划](go-java-v1-raft-commit-timeout-plan.md)对应[055235主证据](evidence/local-go-java-raft-commit-timeout/README.md)：3000ms选举配置，原SDK一次Begin/Commit零Rollback，自然16；5TC JVM/3Go SDK、两次实际领导更替、两次DB/SDK重启、真实TCC确认10成功4丢失、AT清Undo只余TCC，再次同分支持久重放效果仍1，最终全三副本会话/分支/锁清理。

运行87 Python（13归一化、33原始业务、25集群/partial、3输出、9完整副本、4停止身份）；额外原11/12和13/14到17条各副本分支status/context直接绑定4，先RED3失败/1控制后全绿，合计91。948运行输入＋3最终验证输入、341classpath/2821依赖、同20生产class/同3只读辅助class，20关键类来源、六snapshot/两原盘加载/两JFR重读、五原生查询30帧与原RPC/SQL/Undo/控制台分别复核；资源停止、11端口释放。Go254/Rust117/Java RAFT52继承无新执行。

054436业务完成但单项测试导入残留导致86/87，修后pilot数据87绿；054700 1000ms在第二次选举60秒无唯一稳定领导，保留失败OPEN。正式055235独立3000ms通过不关闭1s问题；因果仍推断。11==13不变context路径已认证，Prepare后修改复制仅静态来源风险未验收，见[发现](raft-commit-runtime-findings.md)。原reload/metadata/logging、指标/Exporter、全SQL/分区/并发/损坏/崩溃、生产认证、正式Rust、完整WP00与Phase1–7均继续开放。


## RAFT Prepare 上下文更新修复候选（2026-11-13）

[计划](java-raft-context-replication-candidate-plan.md)对应[原实现真实缺陷](evidence/local-go-java-raft-context-reference/README.md)与[0005候选064040](evidence/local-java-raft-context-candidate/README.md)。原11/13确实不同，原副本保留旧上下文，选主后实际TCC拒绝确认、业务效果0；负向运行不计恢复成功。候选统一补丁组三节点在两次领导更替、六snapshot/两原盘加载、两次DB/SDK重启及真实成功回复丢失后，同原分支持久重放确认仍10/效果1，最终三副本清理，余额900/Undo空。

直接绑定原13/14→12副本观察→全部真实TCC回调（阶段5/1/1）→五SQL准备token及两个接受记录；5TC/3Go SDK、35实际类来源、两JFR重读和五原生查询30帧完整核验。Java同字节25测试原类RED11失败/14控制、候选25通过，真实原类705B旧消息兼容通过。Python102＋追加实际证据破坏10＝112；归一化15已包含，空guard适配器RED仅作校验器负控，不冒充原生产RED。1039运行输入＋10追加验证输入、341classpath/2821依赖、23生产覆盖类/1只读控制/3辅助类前后闭合；资源停止、11端口释放。Go254/Rust117及旧Java父范围继承未重跑。

JAVA-RAFT-BRANCH-CONTEXT-001原参考仍OPEN，独立0005取得选定范围证据；只认证统一补丁与新reader读旧消息，不支持混合版本滚升声明或修复历史已丢上下文。1000ms分票、metadata/logging、指标/Exporter、全故障矩阵/通用Fence/认证、正式Rust、完整WP00和Phase1–7继续开放；分阶段冻结提案仍待确认。


## Java / Go AT 共享锁实际缺陷（2026-11-13）

[计划](java-go-at-shared-lock-reference-plan.md)对应[070224负向主证据](evidence/local-java-go-at-shared-lock/README.md)。真实Java与Go AT在同一MySQL行同时保留已提交阶段一，余额1000→900→800、两个原Undo并存；固定Java锁键仅换resource即可看不到原锁，资源命名空间隔离由实际查询证明，锁键文本差异仍单独开放。GO-AT-RESOURCE-ID-001由源码推断升级为所选组合实际锁绕过，未认证安全互斥或迁移。

32 Python=归一化12＋实际证据破坏20；后者旧冻结校验器RED18失败/2控制、新门禁20通过。118运行/32追加输入、342classpath/2802依赖、16类来源、6原关键源/类、12原生查询72帧及真实TM/报告/原Undo分别闭合。Java客户端默认V2、Go V1原样记录，未扩张V1认证边界。原TM提交清理后余额800/Undo空/锁释放，3自有进程及容器退出、四端口释放。Go254/Rust117单元继承无新执行；正式Rust、WP00和全计划其余门禁仍未完成。


## 2026-11-13 — 全局状态有界契约验证

[正式运行130921](evidence/local-global-status-contract/README.md)：Java21状态/9分类189观察/441校验对（108拒绝），Java四种响应168例在原Go及既有0001–0006候选字段/字节相同；235Java未知枚举拒绝，940未知响应变体原Go保留/显式入口拒绝。新执行22 Python；11语义门禁RED10/1、5来源集合RED4/1、6真实副本RED2/4，后者为验收前读取execution元数据而调整夹具，前后源均保留。所有RED是证据门禁对照，不是上游生产RED。

28冻结输入含22 Python动态闭包，Java2122/原Go714/候选24+辅助13、278classpath/1950文件前后及封存时重核；16class绑定认证原JAR/原target/实际来源，Probe启动前SHA及两Go binary、编译参数/版本归档。92文件catalog SHA256=1ba8ab9c1849c3d09b43bc67d5335b1ad5e76a32cd63404872e762027943f4eb。125821/130032/130514保持诊断性质；未补写旧输入或新增生产补丁。GO-V1-STATE-001、完整业务状态与WP00仍OPEN；本项未启动网络服务。


## 2026-11-13 — WP-00恢复调查完成度核对

按原任务允许的“调查或明确受阻”完成[REC001–010逐项核对](wp00-recovery-investigation-closure.md)。[只读审计131844](evidence/local-wp00-recovery-investigation-audit/README.md)新核对11份历史manifest/20,880列出文件及关键失败、种子组合、真实DB补偿/提交重放事实，catalog SHA256=770e87dca4e4ec8516c2348a5704ce3b44a96eab8b733418643280c60c9bf633。没有新业务运行/Java-Go-Rust测试，也未重归档旧证据。只勾选调查checkbox，原版失败、未执行项、候选范围、安全恢复/WP00/Phase0及正式WP01依赖均明确保留；只读复审指出的原RAFT人工重试未执行和历史状态歧义已修正。


## 2026-11-13 — 原候选29载荷/6帧重新验证

[正式133856](evidence/local-wp00-vector-audit/README.md)新执行原Go29/6、原Java29载荷＋6帧的35独立断言体（未用JUnit引擎/Maven）和Rust117；有效语义timeout变异Java1fail/34controls、同Rustbinary1目标fail。文档25行及29binary/manifest/JSON匹配，原语料零差异。原JAR优先解包库、475classpath JAR/158实际原类/16动态类分类、643启动前输入含外部XML、Java/Go/Rust产物闭合；catalogSHA=ad0789a861cba04070abecd7bd148761177ce1fe7fc7e388d77cbfdd5cdac153。早期SPI配置/Windows长度/动态类诊断和133431缺XML输入不改标最终闭包。本轮未修生产代码，不把原29/6缩成any任务全部范围，原checkbox/补充差异/WP00/正式WP01依赖保持原状态。


## 2026-11-13 — 明确批准V2与正式Rust基础实现

真实用户答复“批准修订版及首期范围”已记录，V2已应用、首期allowlist已接受，WP00-P核对1732不可变存档文件/29原夹具，两个历史latest_run可变指针显式排除并记录而未冒充原运行。正式workspace现含crates/seata_protocol，原独立probe在exclude内保持独立。

[基础正式运行](../phase-1/evidence/protocol-foundation/README.md)12测试在Rust1.99/最低1.85各通过，fmt/clippy/doc warning门禁通过，旧probe117测试+fmt/Clippy新跑通过；650冻结输入/673文件catalog SHA256=47152102d5213b6ce309f0c15a00d9c165f5969e075d80891d62b0cf2bf1d980。仅完成WP01域枚举/显式映射/字段原语三项，未完成全部WP01或客户端。外部TC风险/完整WP00-R/后续全部验收不因顺序批准减免，共享锁保持延期。

## 2026-11-13 — 正式25普通载荷模型与编解码

[正式运行164538](../phase-1/evidence/protocol-payload/README.md)新跑53独立Rust测试（7+5+30+8+3），1.99/最低1.85均通过；fmt/全部target Clippy -Dwarnings/rustdoc -Dwarnings通过，原独立probe117及fmt/Clippy新跑通过。只读复审无阻断，四个注册文件已按register_rm/register_tm命名。689输入/721文件catalog SHA256=fe6846e339c5896a8fc9b5ef9b3d8803a081b3c07c76844f21097a082ad422cc，含两工具链正式测试binary及原探针binary。TDD RED为缺API编译失败，非53运行期失败。

正式API比对原29payload独立语义预期及425既有真实Java输出（250异常/168状态/7补充失败）；索引哈希、固定pin及原记录逐字节派生只读核验通过，没有新Java执行或把Go差异改标兼容。25普通类型实际支持，59/60显式未支持而批准范围保留，原nullable/codec/完整向量checkbox仍未完成；完整WP01及所有后续门禁继续开放。

## 2026-11-13 — 正式59/60聚合与树预算

[正式运行165828](../phase-1/evidence/protocol-merge/README.md)新跑61独立Rust测试（7+5+30+8+3+8），1.99/最低1.85均通过，fmt/Clippy/rustdoc拒绝warning；原probe117及fmt/Clippy新回归通过。758输入/795成员catalog SHA256=87b8f709aab5e49f36b5ec0c56dd0e4991759a74d907d742d4bba6ceffb0e37d，两版Rust实际binary归档，四固定Java聚合源/HEAD无差异。

44既有实际Java聚合payload（6控制＋38扩展）与原封存manifest/JSON/TSV/二进制一致；三个旧心跳夹具不计正式frame测试。普通29/425记录继续验证，无新Java执行。API缺失RED为编译失败，复审预算顺序另有真实运行RED1及修正后GREEN，未混称61个运行RED。批准27类型codec实装，仅消息codec任务新勾选，WP01现4/9；其余帧/nullable/全长/向量、原Go差异及全部后续门禁仍OPEN。

## 2026-11-13 — 正式完整V1帧、nullable头与全部既有协议语料

[最终172217](../phase-1/evidence/protocol-frame/README.md)新跑74独立Rust测试（7+5+30+8+3+8+12+1），1.99/最低1.85均通过，fmt/Clippy/rustdoc -Dwarnings；旧probe117与fmt/Clippy新回归，4Python类来源门禁通过。1148输入/1192成员catalog SHA256=2993ad4fa4dea37abf3b81a4da4ab698eade391830f59090c39e7f02da800815，两版Rust实际产物前后核验。头key预算顺序真实运行RED1后修正；API RED为编译失败，Python分类helper RED为导入失败，各自性质独立记录。

本轮新Java严格171558实际37帧原encoder/decoder字段核对（无JUnit/TC），159原class/7关键来源/475依赖/9输入与原JAR闭合，184成员catalog=ebd29a04818c319bb1d8c805e84b25f9ed1ddd2717ceb4133a992e420e6a4b4a。171203是来源分类未收紧的实际37帧中间运行；171139为JVM启动前失败。原夹具manifest保留、同TSV新增严格certification，370历史成员逐项验证。正式最终脚本只读引用这些Java捕获，未重跑Java；172127已通过相同测试，但执行次数字段含混，由172217明确分开。

原29payload/六frame、425记录、44聚合/九控制frame、78独立handler侧车和新37完整frame均在正式API通过。九项实施任务可勾选，完整WP01 Gate仍OPEN以收敛全部peer-profile比较与显式Go适配证据；不将原Go差异、未知/未支持105/106/121、未编码null-report或严格拒绝策略改称原Java/Go互操作通过。TCP流、正式连接/TM/RM/资源恢复及WP00-R/后续所有Phase继续推进。

## 2026-11-13 — WP01批准初始peer-profile完整字段/字节门禁

[正式174850](../phase-1/evidence/protocol-peer-profile/README.md)新执行正式Rust/显式Go两个真实程序比较628正向（576payload/52frame）、2050负向，均符合独立Java实际字段/构造规则与侧车；本轮无新Java运行。三类Java只读来源工具和原/候选Go审计均通过，原Go714/候选714基线+24条目+13辅助、1936Go编译依赖前后相同，默认registry未由程序新增59/60。

正式74＋验证器注入encode-error单测1在1.99/最低1.85分别通过，fmt/Clippy/rustdoc拒绝warning。7Python比较器与12真实输出破坏负控通过，后者是证据门禁测试，不是生产RED。P2错误分类真实RED1后修正，decode失败才rejected、重编码失败encode_error单列；最初比较器RED为缺模块导入失败。1847输入/1914成员、18测试binary和2peer程序归档，catalogSHA=8a5ba6f717bb2f9be1a01b7830620304b3378e3c8aa1ae6918a0f42c3c577fdb。只读终审逐SHA、字段/字节重放和负控闭合，无剩余阻断。

依[门禁记录](../phase-1/wp01-protocol-gate.json)关闭已批准初期Java-authority＋显式Go适配profile的WP01协议组件Gate，WP02依赖满足。不关闭原Go默认差异/完整Go兼容、WP00-R/完整V1/发布与全部后续退出条件；未新跑Go254/旧probe117或现场TC，外部TC风险未接受，共享锁仍延期。

## 2026-11-13 — 正式transport流/连接内ID基础

[正式182417](../phase-1/evidence/transport-foundation/README.md)新跑11transport测试（1unit＋10integration）在1.99/最低1.85各通过，workspace fmt/Clippy/doc warning门禁、原probe117及fmt/Clippy新跑。718输入/741成员、5实际测试binary，catalog=ab4ff9fd23accc104dc0ec2962439a8e58ffbcc6fe620f1b734c41bef1902f18。46既有Java认证帧流分段/单字节、1000coalesced预算、partialEOF/终止/先好后坏、metadata容量和ID正负环绕/唯一/额度验证；满空间测试仅模拟合法已发2^32-2状态的尾两次，不计作四十亿次执行。

协议共享header修改后，新peer181728实际重跑74协议+1验证器/MSRV及628/2050，1848输入/1915成员catalog0894066182ecae3458b9804a43318f01031b3cb4ffc7d0422c1707e2c0ff32a4。69实际编译/Cargo输入与transport相同，子封存成员完整；仅README后续文档更新，不声称旧全部输入当前零漂移。181302旧workspace固定74计数门禁的诊断已保留，包级作用域修正后通过，不称其为产品RED；新增API初始RED为编译缺API。

只读终审无阻断，只完成WP02两项实施任务，未完成TCP/pending/写入结果/NET007整体或WP02 Gate。无本批新Java/现场TC/DB或新第三方依赖，完整Go兼容/WP00-R/全部后续阶段与共享锁延期边界不变。

## 2026-11-13 — 正式TCP单会话、等待表和结果分类

[正式191253单会话门禁](../phase-1/evidence/transport-session/README.md)33项在Rust1.99/最低1.85分别通过：9unit＋10流/ID＋14实际loopback TCP测试函数，workspace fmt/Clippy/doc严格通过。期限复审问题有真实RED1/GREEN和两版本全量回归；接收端在同一仲裁锁内检查绝对期限，不依赖调用future的timer被及时poll。初始API RED仅编译缺类型，未计九个运行断言失败。只读复审确认修复、未见新阻断；并未重跑服务。

连接私有等待表、独立checked generation、同socket不重用ID、有界队列、按项write marker、响应类型/完整批次guard、取消/迟到/重复/断连、显式shutdown/Drop与unpolled-worker/poison终止均实现并验证。两个活动socket同ID及signed-wrap逆序并发有真实TCP断言。catalog=bde63f35709b2ef4cf9472d3a65a998424cc13623039815d7683ff9a92833fe3，2546来源/2570成员/6实际测试binary，16registry依赖archive checksum与实际解包源码封存。新锁定Tokio1.54.1及依赖均使用现有缓存，D/E写入，无网络安装。191226仅metadata缺缓存诊断，不是完整成功证据。

新peer191025重新执行正式74＋验证器1在双版本、628正向/2050负向、7Python/12实际输出负控，1848来源/1915成员catalog=e89f562278c942a3de7af0575357a93e66045dd563b992d25ef62109cf2b5c87。实际协议/Cargo编译输入相同，子归档全部哈希核对；后续README/清单更新不称旧全部输入当前零漂移。本轮没有新Java、旧probe117、Go254或TC/MySQL业务运行。

WP02仅7/12；NET001–007只完成传输组件场景，连接池、定时心跳/idle、重连退避、认证/RM ready、NET008–012与整个WP02 Gate仍未完成。所有后续阶段/Go默认兼容/WP00-R保留，共享锁延期、外部TC风险未接受。

## 2026-11-13 — 正式心跳调度与完整帧读空闲

[192909心跳门禁](../phase-1/evidence/transport-heartbeat/README.md)新执行47transport在Rust1.99/最低1.85分别通过（14unit＋9heartbeat target＋10stream/ID＋14原TCP，共22真实TCP函数/1配置测试），workspace fmt/Clippy/doc拒绝warning。API初始RED只缺接口编译错误；四项独立于正式代码的隔离行为变异控制都触发实际断言失败，不称四个原产品故障。它们复用一个D副本逐项从基线重建；business/PONG删除同一reset但用不同实际目标测试。首轮审查指出的业务/PONG/skip接线覆盖已补齐，复审无阻断。

调度不增加后台timer任务，PING共享ID但不占pending，读期限检查完整有效帧、半帧滴流不保活；业务/控制写重置与skip不重置、跨九窗口PONG保活、静默失活清理、额度与关闭/Drop均有实际测试。2556来源/2585成员/8实际binary，catalog=bd05a0602c9e4172e50affc6bd159c186227ea1407b4b19072367598e57caaf9。只读终审重算全部成员/产物、16archive checksum及1850包文件、五Java来源与pin、70协议/Cargo输入和负控SHA，CIM未见本轮自有进程残留。后续文档更新不称全部旧输入当前零漂移；正式编译源保持一致。

新peer192819完整执行74协议＋1验证器在双版本、628/2050、7Python/12真实输出负控，catalog=b8ddd7aac9ee5c776a9268f74bb1ffeec09c7939907aaf905b50438bcf84e9e2。dev-only test-util已启用，16锁依赖不增加且离线使用现有缓存。无本轮Java/TC/MySQL、旧probe117/Go254、安装或网络获取。

只关闭心跳/idle任务，WP02为8/12；pool/reconnect/authenticated RM ready、NET008–012、完整V1/全部后续阶段仍开放。既有Go默认差异/WP00-R、共享锁延期、外部TC风险未接受不受此门禁改变。

## 2026-11-13 — 正式raw池与代际重连

[195040连接池门禁](../phase-1/evidence/transport-pool/README.md)62在Rust1.99/最低1.85各通过（19unit、10pool、9heartbeat、10stream/ID、14原TCP；31真实TCP函数/2cfg），workspace fmt/Clippy/doc严格通过。固定槽位/100并发获取、原代际入站有界/溢出关闭、旧handle隔离、丢Begin响应绝不自动重发、实际指数上限/稳定重置、shutdown取消建连/唤醒acquire、ownerDrop与unpolled guard均有验证。

复审两P2都有实际运行RED：quota错误未关闭导致耗尽通道永久被选择；锁等待后仍保留完整5s预算（实测4.999514s）。修复为同步关闭额度耗尽代际并保留原Local错误、在检查锁前建立并跨pool/session传递唯一绝对Instant；61 GREEN后补shutdown-connect，最终62双版本通过。五隔离变异均为目标断言失败exit101，排除编译错误，不称原产品RED；同一个D副本每项从基线重新构造并最终恢复。

2564来源/2599成员/10实际binary，catalog=b326692a210b89b945a73b3cd53406de0da0b2ac18fd02195c1aaa9389774586。只读终审核对所有SHA、16archive checksum、1850依赖源文件、负控SHA/实际日志和恢复基线，未见本轮自有进程残留。协议70编译输入与既有192819相同，628/2050及其74＋1/7Python/12负控为引用证据，本轮没有新跑peer或Java；后续approval/checklist/README更新不算编译漂移。

仅关闭raw pool/reconnect两任务，WP02为10/12。身份/缓存资源每新会话ready、NET008–012、完整WP02/V1和全部后续阶段仍开放；无新依赖、安装、联网、TC/MySQL或旧probe117/Go254运行。共享锁延期、Go默认差异/WP00-R与外部TC风险未接受仍保持。

## 2026-11-13 — 正式原生登记/资源修订Ready与JavaTC现场

[202518正式门禁](../phase-1/evidence/session-registration/README.md)76在Rust1.99/最低1.85各通过（20unit、10pool、9heartbeat、13registration、10stream、14原TCP；42实际TCP函数/4config），workspace fmt/Clippy/doc严格通过。动态typed缓存、首资源RM身份/资源同attempt、明确拒绝/Local/Unknown与后续NotAttempted、整轮100msdeadline、改缓存取消旧初始化及late回复、每代际全量登记、关闭/Drop均验证。

API RED仅缺接口编译错误；资源型auth兼容问题有真实断言RED，移除额外空资源RM握手后GREEN。六隔离行为变异从相同D基线逐项重建且目标断言均失败exit101，不称六个原产品缺陷。2590来源/2628成员/12实际binary，catalog=38f8255d69b57f15eaeb43ea05a3b7927c079d76014296e8eff9eae75437f107；16依赖archive与1850包文件逐项核对。

[Java202129](../phase-1/evidence/registration-java/README.md)实际新启动原固定TC并使用既有公共认证SPI：TM/RM接受/拒绝、两个资源的新代际全登记、动态第三资源拒绝保持不可选六场景通过。133来源/146成员/实际正式client binary，catalog=3c6bcb2027de70623779f9eaa942dae9b831fba1732fed5aa817f5780622224f。class-load实际5来源（3core原JAR、AbstractAuth原JAR classes、checker冻结runtime扩展）、两实际runtime SPI文件和历史认证hash、122共同编译输入、store0及自有TC/38091清理均核对。先前201818同六功能用例仅作来源较弱的历史成功，不叠加为十二种场景。

父门禁脚本没重新启动Java，live-java-reference明确本里程碑有新的Java子checkpoint执行；无新Java编译/JUnit。协议peer192819仍只复用70相同编译输入与628/2050等归档，未新跑。只读终审全部来源/产物/负控、子归档、现场断言与类来源通过，无残留；后续README/进度更新与编译源码一致性分开。

只关闭每会话资源登记任务，WP02为11/12；NET008/009注册组件验证通过，NET010–012/完整WP02、TM/RM业务、MySQL补偿与全部后续阶段仍开放。生产认证/TLS、资源删除/持久去重、外部TC风险未接受、原Go默认/WP00-R和共享锁延期边界保持。


## 2026-11-13 — WP02正式原代际回包与NET001–012组件Gate通过

[组合212118](../phase-1/evidence/transport-recovery/README.md)87正式transport测试＋另3联调辅助测试在Rust1.99/最低1.85各通过；24unit、49真实TCP函数、10stream/ID、4config。workspace fmt/Clippy/rustdoc严格通过，14隔离行为变异均实际目标assert失败exit101；13Python含13原始帧/状态破坏被拒绝。2634来源/2695成员/16实际执行binary，catalog=ba1458a8a1e944e4693a552fe6ef201db5b4b2457646170e5aca70637006c5f2；16依赖archive及1850包源码核对。

PoolInbound消费原代际能力，支持原signed ID/nullable有序头/精确XID与BranchId的sync回包；独立有界回包额度与普通future表隔离，同ID不交叉完成。完整本地写完才成功，写前取消跳过整帧，已开始写必须完成或关闭socket。实际产品RED发现关闭先发生仍发布成功、无Tokio上下文却已入队；修复后双版本GREEN。观察器u16布尔值与raw关联/运行配置/成功发布次序缺口亦有RED/GREEN，早期诊断不计最终认证。

[登记Java211350](../phase-1/evidence/registration-java/run_20261113_211350_6a0bee84/result.json)六现场用例重跑通过：139来源/152成员，catalog=8ac6fd9f4a2c2e1af44be92ea9f3ed9a1bc35479e73b0fd89815cacb8fe2487f。新[丢响应Java211914](../phase-1/evidence/transport-loss-java/README.md)三个独立TC＋三个代理通过：143来源/185成员，catalog=76219d90eeb6cdde8a7ab7d67fddf083815545d91af6bfb83f71a5ef8abe8519。NET010显式查询/同XID同操作重复Commit，不转Rollback，实际Finished15/15保留confirmed=false；NET011只发一次Begin、客户端无观察器XID；NET012不重注册，仅从真实匹配rollback callback学原BranchId、TC明确Rollbacked11，模拟动作一次/锁释放。

只读终审核对父/子catalog、各128共同编译输入、26实际Java类来源、三份实际运行配置、56原始帧和状态顺序/元数据。伪造客户端PASS而保持原wire15/15被父审计拒绝。所有自有TC/代理及38091/38092清理。父脚本自身没启动Java，引用本里程碑新Java子运行；未编译Java/JUnit/MySQL或重跑旧probe117/Go254。协议peer192819的70编译输入一致，628/2050等仅引用未新跑。发布README/清单/approval是执行后更新，不宣称所有旧文档来源当前零漂移；当前编译输入不变。

[WP02机器状态](../phase-1/wp02-transport-gate.json)已验证，清单12/12，下一步WP03正式TM API/异步上下文/期限及未知结果协调。范围仅已批准首期transport组件及所选NET故障场景；生产TM/RM、持久幂等、SQL/Undo补偿、生产认证/TLS、原Go默认兼容/WP00-R、全部后续阶段与发布仍开放。共享锁继续延期，外部TC风险未接受。无新依赖、安装、联网或分支操作。


## 2026-11-13 — WP03正式TM组件、来源约束恢复与基础Java验证

[221732组合证据](../phase-1/evidence/tm-component/README.md)28TM测试（7配置/上下文、13调用/恢复边界、8作用域）及87传输回归在Rust1.99/最低1.85各通过；10隔离行为变异都触发实际目标断言失败exit101。workspace fmt/全target Clippy/rustdoc严格通过。2058输入/2122成员/22实际执行binary，catalog=a41801b7fcad2ba66f28dd99046c5f62a5f45ad7248621e9c4fa4a5dfaeaeed8；16依赖archive及1850解包源核对，无新依赖/安装/联网或分支操作。

正式seata-tm包含登记TM池builder、独立网络/TC期限、精确XID/可选字段、词法task context、六传播策略、Begin/Commit/Rollback/Status和完整Typed错误。业务错误与回滚错误同时保留；UnknownCommit不转Rollback、不自动重发；Finished不能确认历史提交，失败查询撤销旧许可并保存TC retry-timeout状态。panic/取消恢复本地上下文，不启动无所有者的网络清理。作用域错误可用本管理器真实Launcher签发的私有RecoveryToken恢复原XID/原意图，改错误字段、伪造诊断、跨管理器均拒绝；先查询且不申请新TC期限。

接口首个RED为缺API编译错误。后续真实断言RED修复附加数据/名称Debug泄露、Failed/Finished擦除原未知、失败查询沿用许可，以及公开诊断可切换Commit→Rollback/伪造恢复能力。两份来源RED后加入私有manager/XID/op证明，负控验证忽略issuer仍会被测试抓住。早期220916虽然27测试/9负控通过，但恢复来源P2未关闭，仅作历史诊断，不能替代本最终组件记录。

[Java221553](../phase-1/evidence/tm-java/README.md)使用原固定TC/JAR及公共认证SPI，六功能场景/七个空全局事务实际通过；167来源/179成员/catalog=dcbb3a1ed9c0671dfc9f50a323b84c15356114dc9e72e1f8df0150d8b8831e5d。原始stdout字节7Begin/4Commit/3Rollback，七XID/nonce/50ms与60000ms期限、6实际类来源/实际配置/SPI、client binary和自有PID54740/38091清理均核对。Java提交确认后的异步清理用至多5s只读状态查询观察，七个最终Finished；没有重发事务或以Finished倒推历史提交。初215516仅收尾立即Finished假设失败，不计成功；其后历史成功因源码更新均被最新221553取代。

新[peer215647](../phase-1/evidence/protocol-peer-profile/run_20261113_215647_eee62e75/result.json)在新TM工作区实际执行74协议＋1验证器双版本、628正向/2050畸形、7Python/12实际输出破坏控制；1849来源/1916成员/catalog=23b5d8072aee9037757604763d5c073164dc564394f97f9702e3418a2c817919。71协议/Cargo输入与父一致，显式Go0001–0006实际API运行，原Go默认兼容未关闭。未执行新Java编译/JUnit、MySQL、旧probe117/Go254或再次完整旧transport负控；Java子运行是本里程碑新执行，父脚本自身不另启动TC。

只读终审核对全父/子catalog/源快照、22产物、155共同Rust/Cargo输入、16archive/1850依赖文件、10负控patch/恢复副本及Java日志/来源/配置/清理通过。随后发布README/清单/approval/状态不宣称旧文档当前零漂移，编译输入单独保持一致。[WP03状态](../phase-1/wp03-tm-gate.json)记实施组件12/12，**完整Gate仍OPEN**；下一步正式TM对原JavaTC的真实响应丢失/不确定性恢复，不能以旧transport驱动代替。RM/SQL/Undo、跨进程持久恢复、生产认证/TLS、原Go默认/WP00-R、全部后续Phase与完整V1发布仍开放，外部TC风险未接受，共享锁继续延期。


## 2026-11-13 — WP03正式TM真实响应丢失与完整组件Gate通过

[224638组合门禁](../phase-1/evidence/tm-gate/README.md)绑定28正式TM＋87传输回归和另2联调辅助unit，Rust1.99/最低1.85各通过；19隔离变异（10库＋9观察器）均真实目标断言失败，13观察器＋7proxy Python通过，workspace fmt/Clippy/rustdoc严格通过。2090输入/2175成员/24实际测试binary，catalog=f59fdd96a66d2a1b8768a9c0317bbe1464fe97e7797847ba66798bb11f92e4d2；16archive/1850依赖源逐SHA核对。

新[Java224303](../phase-1/evidence/tm-loss-java/README.md)三个独立原TC＋三个代理，正式execute/recover_unknown路径的Begin/Commit/Rollback成功响应丢失通过。175输入/216成员/catalog=76f1949973528f422db0a5b9fa583d172f47eabcff935736ba7031ecd2824f63。Begin无XID、不构造业务、不再次发起；149字节FILE证明原执行，观察器身份未提供客户端。两个结束模式使用真实原分支回调，内存动作各一次/锁释放，同管理器原凭证只恢复同意图、查询后显式重复一次；实际15/15仍unknown_retained=true，Rollback原业务错误保留。58原始帧，唯一实际响应drop、原15请求在drop后且16在retry前；没有改写帧或无依据声称历史成功。

首轮223750仅功能成功，验收器误允许重复投递原响应和旧在途查询冒充恢复；新两方法出现实际四次断言RED，修后13方法GREEN，新epoch重新跑224303。旧support及正式TM库未改。九D独立Python验收器削弱变异全部被反例断言抓住，不能把API初缺模块红误称运行失败。

引用此前Java221553六基础场景/七空事务及6类来源，155共同编译输入一致，未在本轮重跑；新fault子156共同输入，仅新增driver，原库不变。基础6＋故障3×7=27实际类来源、原配置/实际runtime配置/SPI/JAR/client产物和清理均核对；父自身不启动TC，子本轮确新启动3TC＋3proxy。未新编译Java/JUnit/MySQL、旧probe117/Go254；协议peer215647的71相关输入及既有628/2050、74＋1双版本/7Python/12输出负控只引用，未冒称本轮重跑。

只读终审父/子全catalog、155/156/71编译输入、24产物、16archive/1850源码、19patch与候选恢复、58帧/来源/配置/SPI/清理通过，无本轮进程及38091/38092残留。发布README/清单/approval后另查编译输入，不宣称所有历史进度文档零漂移。[WP03状态](../phase-1/wp03-tm-gate.json)已验证，12/12实施与完整所选TM Gate关闭，下一步WP04正式RM控制面。

该Gate未完成生产RM分发/持久幂等、SQL/Undo、生产认证/TLS、跨进程恢复、原Go默认兼容/WP00-R、全部后续Phase、完整V1或发布。共享锁继续延期，外部TC风险未接受。没有安装、网络获取、新依赖或分支操作。

## 2026-11-14 — WP04正式RM持久恢复基础已验证，完整Gate继续

[最终基础记录](../phase-1/evidence/rm-foundation/README.md)新增正式seata-rm资源目录、显式恢复适配器、共享租约分派及有界追加日志。先同步Pending、未决先查询、完成先同步；取消的后台磁盘工作保留分支/预算许可，同租约变更也串行。真实取消断言RED修复旧写晚到窗口；初API RED为缺模块/接口编译失败，容量夹具200→160纠正不计生产缺陷。

Rust1.99/最低1.85各212正式测试（74协议＋87传输＋28TM＋23RM）通过，46实际测试产物；RM为1真实write失败/poison、12日志、8分派及2强杀恢复。每版本提交/回滚各3子进程，效果已同步而RM完成未写时父kill/wait，两个新进程恢复/去重，apply/recover各一次，共12子进程。默认ignored child入口确由父启动，不额外充正式测试。CRC/身份/先Pending/先恢复/完成缓存5D盘隔离削弱均编译并在目标断言失败；RM格式/Clippy/rustdoc严格通过。

最终run_20261113_235728_c0b4952f：2604输入/2723成员，catalog=5b14b429114e2ebcdd6295bf6a48febcecc56ee40b4d48699af75d468d74d17c。22锁定archive及18当前平台依赖1835解包源码逐项比较、前后冻结；其他4只验证archive，不声称其他平台测试。早期235227未冻结依赖源码结束状态，经审查修复后重跑，最终记录不能被旧功能成功替代。只读终审全catalog/46产物/12子进程/5目标负控通过。

新[peer235407](../phase-1/evidence/protocol-peer-profile/run_20261113_235407_74265da4/result.json)在四crate工作区新执行74＋1双版本、628/2050、7Python/12输出破坏控制，1850输入/1917成员/catalog=11e3fd8a6afca7c2e507a3ecc39df07b67dc9b6efb87895ba04f439320ea5bfa；显式Go0001–0006，原默认仍开放。发布后独立审计foundation2035编译/依赖输入、peer815代码输入当前匹配，共享72项一致；历史清单/approval发布变化单独列出，不声称全部历史文档当前零漂移。没有新Java/JUnit/现场TC/数据库、CLI安装或分支操作，全部开发/运行输出在D/E盘。

[部分状态](../phase-1/wp04-rm-foundation.json)不关闭任何完整WP04验收项。下一步拥有网络生命周期的注册RM、BranchRegister/Report/LockQuery、完整原连接回调/平坦批次与Java TC重复/重连Gate；真实资源NotApplied迟到围栏、独立sync故障和同租约操作序列独立负控仍待验证。文件效果夹具不证明SQL/Undo原子性。生产认证、WP00-R、原Go默认、完整V1/发布及其后Phase继续，外部TC风险未接受，共享锁暂缓。

## 2026-11-14 — WP04正式RM运行组件已验证，真实Java Gate下一步

[运行组件证据](../phase-1/evidence/rm-runtime/README.md)实现ResourceManager拥有原生登记池/有限回调JoinSet，ResourceClient单次BranchRegister/Report/LockQuery，完整原请求/TC Failed/未知及协议分类；报告可选资源保持None，不补BranchId/自动重发。原连接signed ID/可空heads、完整flatbatch预校验、原绝对期限、磁盘排空及关闭失败报告已接入正式资源分派。已有初始化期间有界缓存回调、完整revision Ready后转发/发业务的合同保持不变。

Rust1.99/最低1.85各234正式测试（74协议＋87传输＋28TM＋45RM），52实际产物。RM为3unit/12journal/8dispatch/2进程恢复/5calls/8callbacks/7edge；12真实恢复子进程及apply/recover各一次再次执行。11D盘隔离削弱全部编译并在指定目标行为断言失败，含丢清理panic、Failed覆盖、非法成功ID、发送未知标记、批次方向、校验期限等。格式/全target Clippy/rustdoc严格通过。

源码审查两P2已修：关闭收尾吞掉资源Future析构panic→真实目标RED后保留非取消JoinError并清理后Err；整批期限误从parse后重启及超限app先复制→入口deadline/借用校验/seen索引，2边界测试及期限负控。初native API缺类型编译红、时钟目录碰撞夹具修正分别记录，不混称生产行为缺陷。动态资源初试验错误要求TC登记ACK前等待回调，冲突于既有合同；试验泵循环已逐字节撤回，不变更核心标准。最终PONG仅证明reader进度，最后ACK后原回调/单次业务各出现，不能夸称dispatcher调度屏障。

最终run_20261114_003214_ee6afbb3：2615输入/2756成员，catalog=66e1711307ef6a78c94ed1bde0acdd9e1ce1492bb035d6fbf133d9599490cad4。22archives、18当前平台1835实际依赖源前后冻结，其他4仅archive身份。只读终审全catalog/source/52产物/11目标失败/四组原身份CRC状态效果/12子进程均通过，无残留测试进程。发布后独立审计2046编译/验证器/依赖输入及52实际产物当前相同，与继承peer235407的72相关编译输入一致；只有RM README和清单发布MD变化，不宣称全部历史文档零漂移。新Go peer/Java/JUnit/真实TC/数据库并未在本轮执行，无安装、分支或C/F写入。

[当前部分状态](../phase-1/wp04-rm-component.json)只完成模拟TC下的运行组件，完整WP04仍OPEN。下一步固定原JavaTC正式RM登记、真实分支注册/报告/锁查询与提交/回滚、重复投递及重连Gate；不能以模拟TC45测试替代。真实资源NotApplied迟到围栏、独立sync故障、同租约操作序列独立削弱、MySQL SQL/Undo与后续全部门槛继续。外部TC风险未接受、共享锁暂缓、完整V1/发布和后续Phase均未完成。

## 2026-11-14 — WP04获批首期验收通过，继续MySQL AT

[合并验收](../phase-1/evidence/rm-gate/README.md)完成原WP04十项与真实Java Gate。正式ResourceManager/ResourceClient与持久恢复实现不变，继承Rust1.99/1.85各234正式测试（45RM）、52实际产物、11实现削弱断言、12恢复子进程；当前2044编译/依赖输入核对相同，不声称本次重跑234项。新增原JavaTC正常/重复/断连各提交回滚六例，6TC/12代理/54实际类来源、96RM与580TM原始帧；资源均apply/effect1，日志仅Pending→Completed并重开，原申请/报告/事务结束均无自动重发，TC负责重试。

双端观察及原TC日志独立绑定nonce、两个原始XID、TC正i64BranchId、原资源/应用数据、报告/锁/结束/回调/交付回包/状态查询顺序。38Python、2辅助资源测试双Rust及10隔离观察器削弱目标断言均执行通过。断连回滚在5秒预算下客户端报告未知；原目标迟到Failed0/exception0/status5保留为独立观察，竞争者回滚必须明确11，所有状态逐项匹配raw16，Finished仅为清理观察；未独立证明精确超时时刻。

Java子014916：2075输入/2169成员/catalog c4763b18ce8143b932683d4a4c95a91fb57b6e7dfd3fabd0cd26147d3d574da2。最终父020104：2076输入/2113成员/catalog3583c327f3fd939a4a5ad007755ac9fa8d317c1cfcd844fe72f8d0612e733ef4。子全部2756+2169成员与快照重新核验；22锁定archives/18当前平台1835依赖源绑定。只读审查发现父控制执行文件CRLF与归档LF的exact-byte差异，已改为归档实际D执行文件原bytes并重跑父；最终10项执行/归档/记录SHA完全一致，旧015811不能作为最终封存。更早单端较弱观察、低分辨率时钟拒绝、合法迟到Failed误拒绝及catalog格式失败均保持历史，不以它们冒充最终通过或产品行为RED。

[当前机器状态](../phase-1/wp04-rm-gate.json)将WP04标记为获批首期控制面通过；SQL/Undo、资源适配器迟到围栏、独立sync故障、生产认证、WP00-R、原Go默认及所有后续Phase继续开放。原初始化回调缓存/完整Ready规则不变，外部TC风险未接受，共享锁暂缓。自有Java/代理与三端口均已清理；没有新Java编译、JUnit、Go/peer比较、依赖下载、CLI安装或Git分支/提交操作。全部开发与运行输出D/E盘。下一步WP05真实MySQL AT phase1，然后WP06及其余原计划。

## 2026-11-14 — WP05 SQL识别首批验证，真实MySQL/Undo继续

[首批证据](../phase-2/evidence/at-sql/README.md)新增正式seata_at有界SQL工厂、三类MySQL识别器与类型化结果/错误；单表INSERT VALUES/UPDATE/DELETE明确矩阵，保存原SQL/表/有序列/行数/绑定数/WHERE偏移，Debug及错误脱敏。真实行为RED覆盖IN列越矩阵、UPDATE FROM与LOCAL/LIMIT ALL被替代解析器消费、未支持运算链先构深树、裸内建项误当列；固定MySQL DIV的坏输入确触发上游unwrap RED，现先限定DML动词并拒绝未支持DIV，不改依赖源码。AST重建逐token等价补足字段为None的检查；注释/DDL/别名/跨schema/join/子查询/函数/变量及未支持形状明确拒绝。

最终023603_0d037bc7：2781输入/2927成员/catalog02382691b9dc5916063eba7147e5622769d9a70c2256c1a7c66fcc6eb39b3902。每Rust1.99/1.85新执行252项（74协议+87传输+28TM+45RM+18SQL）、56真实产物；10个隔离削弱实现成功编译并在指定断言失败，执行/归档/记录SHA相同；4组RM Commit/Rollback恢复与12实际子进程归档。格式/Clippy/rustdoc严格通过。初023055最低Rust回归发现旧TM测试固定20ms睡眠会割断测试端分片ACK写；仅换成有界oneshot后再完整重跑，信号只证明测试端ACK已写完，不声称客户端Ready。生产TM/RM/协议/传输149源未变，31原测试源未变；根工作区只加AT，原锁26包完整保留。

sqlparser0.52.0/log0.4.27由既有C公开缓存读取并checksum核对后复制D，未修改C缓存、无下载或CLI安装；24归档/20当前平台1894实际依赖源逐tar成员核验并冻结。只读终审全部成员/来源/产物/负控及恢复记录通过。Java8对象为PARTIAL，固定sqlparser测试目录85文件仅盘点，源测试案例未展开/未执行差分，所列目录非Java资产0，不声称全项目资产为0或Java完整迁移。

[WP05部分状态](../phase-2/wp05-at-sql-component.json)维持0/14与Gate开放；本轮无真实MySQL/SQLx/Java/Go现场执行，没有镜像、Undo原子提交或提交未知围栏证据。继续真实MySQL元数据/主键与镜像、确定性Undo和同一本地事务，再完成WP06及全部后续原计划。生产认证、WP00-R、原Go默认、完整V1及原生RustTC仍未完成；外部TC风险未接受，共享锁暂缓。所有开发和运行写入D/E盘。


## 2026-11-14 — WP05真实MySQL元数据基础通过，镜像与Undo继续

[元数据基础证据](../phase-2/evidence/mysql-foundation/README.md)新增正式seata_datasource/seata_db_mysql，SQLx0.8.6连接、本地事务、MDL、物理列/复合PK顺序、严格会话验证和单操作绝对期限。失败/取消时poison并丢弃连接；当前不暴露业务提交入口。实际部分撤权成功误判和响应扣留超时RED已修复，初权限夹具/解码失败独立保留。

最终035455_0dec6b55冻结7979输入/8126成员，catalog151350d6ba34f40d03a30521b313a77345ae4fe2f890fb09272854a8423188fc。Rust1.99/1.85各259工作区测试、66工作区产物、4隔离实现削弱目标断言失败、12既有RM文件恢复子进程；另各8项真实MySQL测试和两个独立live产物。现场8.4.13/lower_case_table_names2，覆盖MDL/Drop、不安全表、TRIGGER/PROCESS/部分撤权与43字节响应扣留；两个自有容器已清理。171归档及151活跃依赖6806源码逐项匹配。六Java对象PARTIAL、189源测试仅库存、七资产原字节复制，未新执行Java/Go差分。

[部分机器状态](../phase-2/wp05-mysql-foundation.json)保持原WP05为0/14，WP06未完成。行数据精确解码、镜像、业务SQL、Undo及提交未知仍待实现；当前直接TRIGGER/全局PROCESS权限及loopback非TLS仅为已验证基础配置，不是生产最小权限/TLS验收。WP00-R、原Go默认、全部后续Phase、完整V1及原生RustTC继续；外部TC风险未接受，共享锁暂缓。开发和运行输出只在D/E盘。


## 2026-11-14 — WP05精确行值模型通过，数据库镜像继续

[行值证据](../phase-2/evidence/row-image/README.md)新增Field/Row/TableRecords及精确DecimalValue/FieldValue，完整schema/独立PK序、NULL与空值、65位DECIMAL/30位scale、i64/u64极值和有界原子累积。最终run_20261114_042231_afaee4a5冻结7988输入/8148成员/catalog 8e2ebc241143742e847a3821e7b0e06bf5435d1f8f83112d26ab94340956156c；双Rust各273工作区测试、68产物、10隔离削弱目标断言失败、12既有RM文件恢复子进程。初042039校验器不识别自定义assert提示后修正并完整重跑，不混作产品缺陷。171archives/151活跃依赖6806源码再次核验，锁文件未变。

九Java对象PARTIAL、189测试源仅库存、七资产原字节复制；无新Java/Go/MySQL现场。前轮真实元数据证据保持历史范围。当前五类值域不能证明全部MySQL物理位宽/字符数/UNSIGNED DECIMAL合法性，SQLx解码与补偿前仍须补齐。WP05保持0/14，数据库行读取、before/after、业务/Undo原子提交及所有后续门槛继续；完整V1/发布未完成，外部TC风险未接受，共享锁暂缓。全部输出D/E盘。


## 2026-11-14 — WP05物理域及UPDATE/DELETE真实前镜像验证

[组件证据](../phase-2/evidence/mysql-row/README.md)落实真实物理约束、原参数/原生DECIMAL、同事务长度预算预检与完整持锁前镜像。最终run_20261114_045819_c62f4e63为7952输入/10224成员/catalog 10285b1534c237d5f1eab8944aa039851a70a9798eedd0f80a66bccdb1993ea6；双Rust各282普通+19真实数据库场景，72工作区与4现场正向产物、8普通+5现场目标assert负控、12既有RM恢复子进程。185archives/真实148编译registry的6745源逐项核验；metadata159候选不混称活跃，旧151/实际146另有统计范围澄清。所有原178锁包版本/checksum保留；协议/传输/TM/RM/AT源码不变，旧Java/Go Gate未重跑。

取消现场先证明MDL阻塞、取消后在代理关闭前证明释放；原始43字节只证明响应扣留。general_log核对预算拒绝未读取完整值。九Java对象PARTIAL+两方法引用、189原测试仅库存、七资产原字节。九自有容器均清理，数据目录D盘保留；8.4.13/lowercase2/loopback非TLS，生产TLS/最小权限未验收。WP05仍0/14；两查询间插入/字段膨胀精确竞争、INSERT镜像、业务/after/Undo原子性及后续全部计划继续。完整V1/发布未完成，外部TC风险未接受，共享锁暂缓。


## 2026-11-15 — WP05两查询间并发变化现场通过

[竞争证明](../phase-2/evidence/mysql-image-race-gate/README.md)在完整值查询COM_STMT_PREPARE转发前扣住102B原包，独立BLOB增长及范围INSERT均1205；放行后原一行/一字节，rollback后两操作均成功。Rust1.99/1.85各新执行一例，去预检锁/改RC两实际削弱分别在增长/幻行目标assert失败。最终父run_20261114_052627_abb1eddd为7955输入/8502成员/catalog cac7531795b6296a541b81c61841e9c2552846b87539079dc7f412225d79311a；生产182源与既有223.rs、Cargo全部原bytes，原282+19每版及13负控只绑定继承，不声称新执行。四自有容器清理。

只读终核完整归档通过；D临时目录之后已清理，旧运行源/exe缺失明确记录。已从E封存及C公开索引恢复185archive和6745源码，6930原缓存文件逐SHA一致，无网络/工具安装/Cargo锁变动，不重建旧exe。WP05保持0/14，INSERT/业务/后镜像/Undo原子提交、WP06及所有后续计划继续；完整V1/发布未完成，外部TC风险未接受，共享锁暂缓。


## 2026-11-15 — bounded native MySQL execution/images partial gate

Evidence [run_20261115_205718_cc203af6](../phase-2/evidence/mysql-execution-gate/README.md), catalog `07bed67b72f1501d5dcc7877aa498b1dc0fd186f67696571b00fc14293a45d5e`;7971 frozen inputs/10996 members. Fresh291 workspace+37 live cases on each Rust,80 workspace+8 live positive artifacts,5 pure+6 real weakening controls,12 existing RM recovery children.14 owned containers cleaned;185 locked archives,actual148 compiled registry/6745 source members verified. Literal CAST and pending-count behavioral RED preserved; test BLOB decoding failure and rejected transient source-drift run explicitly excluded from final gate.

Memory Undo only; no public COMMIT/Java format parity/atomic Undo/BranchRegister/unknown-commit recovery. WP05 original0/14,WP06/fullV1/release/native RustTC/all remaining plan open. ExternalTC risk not accepted; shared locks deferred by user. All development outputs D/E.


## 2026-11-15 — native MySQL identity and original format discovery

[Current native gate](../phase-2/evidence/mysql-type-gate/README.md):7992 frozen inputs/9275 members/catalog `fe2928425b3ec7bd3e725f5b104f948548756f0f6f648869e571103fb13fdc6b`;fresh297 workspace+38 live per Rust,84 workspace+10 live positive artifacts,2 pure+2 real target controls,12 existing RM children. Original Java live214333 bound independently:18 original sources/32classes/335runtime files,8.0.27 two profiles x6table cases and16 exact generated assets;5.1.42 two connection failures retained. Not original source-case parity/full Maven environment/Rust codec/Undo atomicity. WP05 original0/14,WP06/fullV1/release/nativeRustTC/all remaining plans open; externalTC risk not accepted,shared lock deferred. D/E outputs only.


## 2026-11-16：选定Undo文档/实际准备日志编解码

[最终组件记录](../phase-2/evidence/undo-codec-gate/README.md)与[机器状态](../phase-2/wp05-undo-codec.json)完成本步：可空原对象图、精确Java类型/数值、有界JSON、真实四准备日志双JDBC配置。最终双Rust各316普通回归、各1个实际codec现场（双profile×4log）、40/40双向Java文档及12个新RM恢复子进程通过；旧每版38个其他DB场景和5+1控制明确继承修复前整轮，不能说最终源码新跑39。两处审查P2有真实RED→GREEN且终核无剩余P1/P2；节点预检原实现正确。185archive/150实际编译包/6871源码未升级，原7资源的checkout-vs-Git行尾来源已纠正并保留原字节。

1135个源测试方法声明仍MISSING，具体运行案例未全部展开，原JUnit未执行、原完整Maven环境未重建。无同事务Undo持久化/真实TC分支登记/提交未知恢复或公开COMMIT；WP05原14项仍0/14、WP06及后续全部计划继续，外部TC风险未接受，共享锁暂缓。下一步在原事务边界接入Undo原子落库与登记/围栏，不能由编码字节推断已完成原子性。


## 2026-11-16：原主键锁键与事务锁集合

[最终锁键组件](../phase-2/evidence/lock-key-gate/README.md)与[机器状态](../phase-2/wp05-global-lock-keys.json)新增原BaseTransactionalExecutor锁键及ConnectionContext锁集合部分；修复主键索引序与原Java HashMap次序混淆、禁止事务内混JDBC配置。两版Rust各325（316既有+9新）测试，1029原方法格式/次序MATCH，实际MySQL每版双配置×9日志锁键（含16列碰撞PK）及4原codec日志MATCH，40/40文档对照新执行通过；8普通/1现场负控命中目标断言，96产物/12RM子进程/3容器清理有证据，终核无P1/P2。178既有生产源与7056依赖输入未变，旧38 DB场景明确继承。

这不是TC锁覆盖/登记、完整Java对象或源JUnit、Undo持久化/原子提交证明；WP05原14项仍OPEN。新增[TC覆盖核查](../phase-2/tc-lock-coverage-investigation.md)记录原来源方法录制后端的未完整资格化诊断：空白键无行仍返回true、分号片段提前结束、byte列表逗号拆分；没有真实TC网络/锁存储结果，不自动接受外部风险。下一步正式验证这些覆盖边界并接入真实BranchRegister、同连接Undo及提交未知围栏。共享锁暂缓、WP00-R/完整V1及全部后续Phase保持原范围继续。
