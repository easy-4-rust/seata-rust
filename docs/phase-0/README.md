# Phase 0 — Architecture & Compatibility Baseline

> **Status: In progress · Research/specification only**
>
> Phase 0 establishes source provenance, protocol facts, state-machine semantics and architecture decisions before implementation begins. No application/business code is authorized by this phase.

**Latest local evidence (2026-11-12):** all 29 frozen samples pass Go/Java/test-only Rust. Seven added failure cases have four compatible pairs and **three OPEN Go/Java divergences**; Rust total81/81, Java target56/56 and 91-module BUILD SUCCESS, static checks pass. [Failure matrix](known-go-v1-failure-divergences.md), [new evidence](evidence/local-failure-variants-20261112/README.md), [prior corpus/optionality evidence](evidence/local-rust-payloads-20261112/README.md). This supersedes older pending sample-validation statements below, without claiming the three divergences resolved, closing Phase 0 or delivering production Rust modules.

**Handler update:**72 handler +7 registration processor cases now pass with the existing suites, selected Java target**135/135** and91-module BUILD SUCCESS. [Evidence](evidence/local-java-handler-matrix-20261112/README.md). Exception/fallback and auth-hook unit coverage is complete for the selected matrix; live auth, full runtime-byte Go/Rust comparison and null-report response encoding remain open. Prior56/73 counts above/below are historical increments, not the current combined total.

**Runtime comparison update:** a fresh135 run now captures all79 named contexts and compares all78 serializable outputs with Go/Rust. Rust**83/83** passes; Go has62 compatible/6 length-difference/1 ordinal-difference/9 opaque-state cases, not a blanket compatibility pass. [Evidence and capture hashes](evidence/local-handler-runtime-20261112/README.md). This closes the pending selected-matrix output comparison, while live auth, known Go/state/null-report issues and full delivery remain open.

**External-loss update:** INT-013–014 now pass for one same-connection external loss each after BranchRegister, Commit and Rollback. No target mutation is resent; registration learns its identity from a validated nonce callback. Finished-only end queries keep the outcome unknown. Final Rust93/93, Python5/5, default and spaced-path live runs pass. [Raw traffic, source hashes and limits](evidence/local-phase09-response-loss/README.md). These simulated-resource checks do not close live auth, retry-timeout recovery or the overall phase.

**FILE recovery diagnostic update:** real persisted retry-timeout records now reproduce full pinned TC startup failure for16 and filtering/lost session-and-lock restoration for17; Begin control passes. Initial Java140/140, final runner fresh focused5/5, Rust96/96. [Evidence and unresolved recovery gates](evidence/local-file-retry-timeout/README.md). Diagnostic reproduction does not close REC-TIMEOUT acceptance.

**DB/Redis recovery update:** eight seeded-state real backend restart/manual-retry cases now pass, including original branch callbacks/locks and empty profiles. Java selected3, Rust98 and Python5 pass. [Evidence](evidence/local-backend-retry-timeout/README.md). Durable RM replay/SQL compensation, live RAFT, natural timeouts and JAVA-METRICS-RETRY-001 remain open; no formal client delivery is claimed.

**Live RAFT diagnostic update:** superseding the earlier pending-RAFT note, three original three-node cohorts complete six leader transitions and three snapshot reloads.16/17 remain visible with original branches/locks, while four exact JFR events prove failed async reload. Java4/Rust106/Python3 pass as diagnostic checks; safe recovery and JAVA-RAFT-RELOAD-001 remain open. [Evidence and limits](evidence/local-raft-retry-timeout/README.md); [findings](raft-reload-runtime-findings.md).

## Goals

1. Pin the exact upstream source snapshots used as compatibility references.
2. Map Java Seata 2.x, Seata-Go v2 and RSeata source entry points to Seata-Rust modules.
3. Specify the wire protocol and identify what is genuinely interoperable.
4. Record global/branch transaction states and the semantic gaps between implementations.
5. Make the V1 architecture decisions explicit and reviewable.
6. Turn source-derived protocol vectors into reproducible Go encoder and Java decoder tests.

## Source snapshots

| Source | Ref used | Commit | Snapshot date | Role |
|---|---|---|---|---|
| [Apache Seata Java](https://github.com/apache/incubator-seata/tree/cba73e180fd8896a99df6415b859d275122feaa7) | `2.x` | `cba73e180fd8896a99df6415b859d275122feaa7` | 2026-10-31 | Protocol and behavior baseline |
| [Apache Seata-Go](https://github.com/apache/incubator-seata-go/tree/82570d905919c1c66bb025a3a5979f25390de498) | `master` | `82570d905919c1c66bb025a3a5979f25390de498` | 2026-11-07 | V1 client architecture and cross-language reference |
| [RSeata](https://github.com/oulover/rseata/tree/c6062776c72a2490f62f8ebb25012c60884f62bd) | `master` | `c6062776c72a2490f62f8ebb25012c60884f62bd` | 2026-03-10 | Independent third-party Rust reference only |

**Pinning rule:** documents must cite commit-pinned paths, not moving `master` / `2.x` links alone. When an upstream snapshot changes, update the source manifest and record what was re-evaluated.

## Phase 0 documents

| Document | Purpose |
|---|---|
| [Source Mapping](source-mapping.md) | File-level source map across Java, Go and RSeata; proposed Rust module ownership |
| [Protocol Compatibility](protocol-compatibility.md) | Wire framing, message types, payload contracts, XID and transport boundaries |
| [Transaction State Baseline](transaction-state-machines.md) | Global/branch state codes and cross-language semantic hazards |
| [Java TC Transition & Recovery Matrix](java-tc-state-transition-matrix.md) | Source-derived state transitions, retries, timeouts, terminal states and restart recovery |
| [Java TC Retry-Timeout Recovery Audit](java-tc-retry-timeout-recovery.md) | FILE/RAFT/DB/Redis retry-timeout recovery asymmetry and REC-TIMEOUT-001–010 regression plan |
| [V1 Payload Golden Vectors](protocol-golden-vectors.md) | Deterministic codec-body and serialized-payload hex vectors, Go verification harness and certification gates |
| [Java V1 Frame Compatibility](protocol-v1-frame-compatibility.md) | Java V1 encoder/decoder source audit, fixed frame layout and runtime interop gates |
| [V1 Control Envelopes](protocol-v1-control-envelopes.md) | Nine heartbeat/merge samples; three heartbeat byte passes and six open Go codec gaps |
| [WP-00 Freeze Readiness](wp00-freeze-readiness-audit.md) | Completed evidence, open acceptance and production-module dependencies without changing gates |
| [Go codec/frame harness](../../tools/go-golden-vectors/verify.sh) | Execute 29 pinned Seata-Go payload vectors, serializer type prefixes and six complete V1 frame fixtures |
| [Java V1 frame harness](../../tools/java-v1-golden/verify.sh) | Execute six Java V1 decoder → encoder frame round trips plus TC exception and recovery overlays |
| [Protocol Compatibility CI](../../.github/workflows/phase-0-protocol-compatibility.yml) | GitHub Actions jobs for pinned Go vectors and Java V1 frame round trips |
| [TM/TC/RM Call Chains](tm-tc-rm-call-chains.md) | Method-level execution paths and uncertain-outcome/error matrix |
| [Java TC Request Exception Matrix](java-tc-request-exception-matrix.md) | Per-request ResultCode, TransactionExceptionCode, status fallback and registration response behavior |
| [Seata-Go Transport Semantics](seata-go-transport-timeout-semantics.md) | Getty/gRPC request correlation, timeout, late response, retry and RM reconnect behavior |
| [V1 Implementation Acceptance Checklist](v1-implementation-acceptance-checklist.md) | OpenSpec-ready V1 work packages, dependencies, scope and test gates |
| [Phase 0 Validation Ledger](validation-ledger.md) | Frozen source SHAs, fixture counts, CI commands and source/runtime evidence status |
| [Java TC Live Integration Plan](java-tc-live-integration-plan.md) | Phase 0.9 topology, isolated FILE TC bootstrap, INT-ENV and live transaction gates |
| [V1 TM/RM Registration Wire Contract](registration-v1-wire-contract.md) | Java/Go RegisterTM/RegisterRM message IDs, body layout, V1 channel selection and response semantics |
| [Java TC Integration Harness](../../tools/java-tc-integration/README.md) | Pinned Java TC build/start/stop/reset and TCP bootstrap smoke scripts |
| [Java TC FILE Bootstrap CI](../../.github/workflows/phase-0-java-tc-file-smoke.yml) | Build pinned Java TC, start FILE storage, and run Rust RegisterTM/RegisterRM against the live TC |
| [RSeata Module Disposition](rseata-module-disposition.md) | Keep/Refactor/Rewrite classification of third-party RSeata modules |
| [ADR index](../adr/README.md) | Accepted and proposed architecture decisions |

## Current high-impact findings

1. Java Seata 2.x defines protocol versions 0, 1 and 2, and the current `ProtocolConstants.VERSION` is 2. Seata-Go's native protocol package declares version 1 and a 16-byte V1 fixed header. Java `MultiProtocolDecoder` selects the decoder/encoder from the incoming frame's version byte; live Run #23 confirmed V1 RegisterTM/RegisterRM succeeds against the pinned Java TC with auth disabled.
2. Seata-Go supports both `seata` and `grpc` transport selections. Its native Seata message codec is a separate concern from transport selection.
3. The pinned Java Seata snapshot contains V1 `ProtocolDecoderV1`, `ProtocolEncoderV1` and `SeataSerializerV1` implementations. V1 support exists in source and the default protocol version is2; Run23 and later local native transactions verify V1 selection/acceptance for their covered paths.
4. The Go `CodecManager.Encode` and Java `SeataSerializerV1.serialize` both prepend a big-endian uint16 message type to the message-specific codec body. The golden-vector document now distinguishes codec body, serialized message payload and full TCP frame.
5. RSeata's `rseata.proto` defines an independent gRPC service contract. It is not the Java Seata native remoting wire protocol.
6. RSeata's current XID construction (`hex(transaction_id):transaction_service_group`) differs from Java Seata's `ip:port:transaction_id` form. V1 must accept the XID returned by TC rather than minting an independent format.
7. RSeata's internal `BranchType` numeric values and its Protobuf enum values do not align with Java/Go ordinal values. Its Protobuf global and branch status enums also omit/reorder values relative to the Java status model. These are protocol correctness risks, not cosmetic naming differences.
8. RSeata exposes an in-memory session store implementation. Its `read_session_by_session_condition` path currently returns all sessions in the inspected implementation; this is not evidence of a durable recovery store.
9. RSeata's `DefaultGlobalSession::is_timeout` calculation compares elapsed time to `begin_time_millis * 1000`, despite storing `timeout_millis`. This is a defect in the third-party reference snapshot and must not be copied.

## Evidence labels

Use these labels consistently:

- **Verified in source** — directly visible in a pinned source file.
- **Verified by test** — a reproducible test has passed and its command/result is recorded.
- **Inferred** — a reasoned interpretation that still needs a targeted test or deeper source tracing.
- **Unknown** — not yet established; do not implement from assumption.

A directory, type declaration, Cargo feature or README claim is not proof of working behavior.

## Exit criteria

Phase 0 is complete only when:

- [ ] All V1-required protocol message payloads have a Java/Go field-by-field mapping.
- [ ] Every candidate payload vector is generated by the pinned Go encoder, cross-decoded by Java and Rust, and stored as a binary fixture with provenance.
- [ ] Java TC V1 protocol acceptance is confirmed against the pinned server with golden byte vectors and a live integration test.
- [ ] `bash tools/go-golden-vectors/verify.sh` passes against the pinned Go commit.
- [ ] Java `ProtocolDecoderV1` accepts the committed complete V1 frames and Java `ProtocolEncoderV1` emits byte-identical frames for the fixtures.
- [ ] XID, BranchType, BranchStatus and GlobalStatus mappings are explicit and tested.
- [ ] Global and branch state transitions have source references and an allowed-transition table; the remaining transition call sites and mode-specific exceptions are closed.
- [ ] TC/TM/RM execution paths are mapped down to handler and resource adapter entry points.
- [ ] RSeata reference modules have a Keep/Refactor/Rewrite disposition; any actual source reuse has separate license and test evidence.
- [ ] ADRs are reviewed and the V1 scope is frozen.
- [ ] V1 OpenSpec tasks are derived from the approved specifications.

**Reference-role clarification:** RSeata is an independent third-party Rust project, not the user's implementation and not the Seata-Rust migration baseline. Seata-Go v2 is the primary V1 engineering reference; Java Seata 2.x remains the compatibility authority. The 29 payload vectors and 6 complete frames are verified by the pinned Go runtime; all 6 frames pass Java V1 decoder/encoder round trips. Live Run #23 confirmed RegisterTM/RegisterRM against the pinned Java TC with auth disabled. Subsequent local evidence verifies GlobalBegin/BranchRegister/phase-two callbacks with simulated resources, test-SPI auth-enabled registration and all29 frozen Java serializer samples. These bounded results do not certify production Rust TM/RM, real SQL compensation or all-message variants; see the validation ledger.

**Validation status:** [Run #42](https://github.com/easy-4-rust/seata-rust/actions/runs/42474150028) passed 29 Go payload vectors + 6 full V1 frames and 17 Java tests (6 frame round trips, 8 TC exception tests, 3 FILE/RAFT reload tests; 0 failures). [Run #23](https://github.com/easy-4-rust/seata-rust/actions/runs/42475353541) passed live Java TC RegisterTM/RegisterRM using the independent Rust client. Auth was disabled for that historical run. Later local transaction/callback and test-SPI authentication evidence is tracked in the validation ledger; production client/authentication delivery remains open.

**Phase 0.7 status:** Java request exception mappings, Seata-Go transport timeout/reconnect semantics, RSeata no-copy disposition and the V1 implementation acceptance checklist are now documented from pinned source. These are source-derived specifications, not runtime interoperability results.

**Phase 0.8 status:** corrected the Go harness source SHA, fixed CI harness issues and the Java zero-length ExtraData expectation. Run #8 passed the 29 Go payloads, 4 Go frames and 4 Java V1 frame tests. Run #18 additionally passed 8 Java TC inbound exception tests (12 Maven tests total, 0 failures). Run #25 additionally passed 3 Java TC retry-timeout unit tests covering FILE filtering and FILE/RAFT reload branches. WP-00 remains open for durable-store restart/operator-retry integration and live Java TC integration; remaining request matrix cases are tracked separately.

**Non-goal:** Phase 0 does not implement TM, RM, AT, XA, TCC, Saga or a Rust TC.

**2026-11-12 payload increment:** all 29 current Go payload samples now pass pinned Java decode, independent object-field assertions and byte-identical re-encoding. The selected combined target suite is 46/46 (29 payloads + 6 frames + 8 exception + 3 reload cases). Binary fixtures/provenance are stored at `tests/fixtures/protocol/v1/`. See [exact evidence and guard failures](evidence/local-java-payloads-20261112/README.md). This supersedes older statements above that all-message Java validation was unexecuted, but only for these frozen samples. The later independent Rust probe verifies all29 frozen samples; a formal production codec and remaining Phase0 gates are still open.

**2026-11-12 control-envelope increment:** nine fresh Java heartbeat/merge frames are checked by typed Rust probe decoding/re-encoding. Original Go Getty matches three heartbeats but lacks59/60 codecs for six merge samples (GO-V1-MERGE-001 OPEN). Java163/Rust114/Python5 and static checks pass; original29 payloads/six frames are unchanged. [Evidence](evidence/local-control-envelopes/README.md), [wire mapping](protocol-v1-control-envelopes.md), [remaining freeze gates](wp00-freeze-readiness-audit.md). No formal Cargo workspace exists; Phase0 remains open.

**2026-11-12 explicit Go patch candidate:** type14/22 error-text lengths have a separately identified fix candidate. OriginalGo21 failures/three controls are reproduced; patched24 cases plus80 codec/bytes test functions and original29/six-frame regression pass. Java187/Rust115/Python6 and static checks pass. Original source pins/fixtures remain intact, upstream incompatibilities remain open. [Evidence](evidence/local-go-java-length-patch/README.md), [patch](../../compatibility/seata-go/README.md). The [WP-00 staged-freeze proposal](wp00-staged-freeze-proposal.md) awaits confirmation; Phase0 and production implementation are unfinished.

**2026-11-12 exception-profile increment:** an explicit Java V1 transaction-response adapter candidate now preserves all25 Java error meanings across ten families. Original152/98 and length-only190/60 outcomes are retained; new profile250/0 plus250 pointer checks and all seven earlier failure semantics pass. Fresh Java437/Rust116/Go104 and new Python6 pass. [Candidate API](../../tools/go-java-v1-exception-profile/README.md), [evidence](evidence/local-go-java-exception-profile/README.md). This is not default Go/transport integration, production Rust or overall Phase0 completion; staged-freeze proposal remains pending.

**2026-11-12 merged-payload increment:** explicit0003 covers25 ordinary types and59/60 containers, including250 error meanings, signed timeouts and explicit optional metadata.38 new/six original merges plus29 ordinary payloads pass; Java475/Rust117/Go110 and new Python7 pass. [Evidence/API limits](evidence/local-go-java-merge-profile/README.md). Getty/frame integration remains a required next stage; default Go and Phase0 are not declared complete.

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

[真实FILE17回滚指标证据](evidence/local-java-metrics-rollback-candidate/README.md)按[计划](java-metrics-rollback-candidate-plan.md)复用0004同一生产class。实际自然17、两次FILE/SDK/MySQL恢复、真实补偿成功丢响应及同分支持久重放均通过；三个独立零基线，初始失败Timer5485ms，恢复重入不伪增完成计数，最终AfterRollbacked counter/summary/timer各1=3ms。8快照/回执、30主类来源及原SQL/Undo/RPC/5原生查询30帧完整绑定。

新执行Python71=业务44＋回滚阶段12＋指标破坏15；Java77、Go254/Rust117/Java617继承未重跑，实际重新编译subscriber与认证字节完全一致。854仓库/40外部输入、278classpath/1950文件前后复核，资源停止且五端口释放。原参考指标问题仍OPEN，仅候选FILE16/17选定范围通过；其他恢复/Exporter/RAFT/并发矩阵与正式Rust、完整WP00保持开放。


## 真实RAFT自然17与持久AT恢复增量（2026-11-13）

[真实RAFT AT证据](evidence/local-go-java-raft-at-timeout/README.md)依据[计划](go-java-v1-raft-at-timeout-plan.md)，将原完整Go AT业务接入已认证前导/保留/reload观察候选。实际自然17、两次可见领导更替、两次业务DB/SDK重启、真实补偿成功丢回复和同分支重放通过；最终三副本会话/分支/锁清理。

5TC JVM/3Go SDK进程、六snapshot/两次原盘加载、923输入、341classpath/2821文件、20关键类来源与五原生查询30帧均绑定；当前任期/peer/成功reload及副本进度、业务查询前后同任期、真实protobuf元数据、原JFR/异常JSON与停止回执分别复核。运行65 Python＋最终停止身份4，共69；Go254/Rust117/Java RAFT52继承未重跑。原参考reload、metadata/logging仍OPEN；本轮仅选定真实RAFT自然17候选，不含指标、自然16、全部故障矩阵、生产认证、正式Rust或完整WP00。


## 真实RAFT自然16混合提交增量（2026-11-13）

[真实RAFT混合提交证据](evidence/local-go-java-raft-commit-timeout/README.md)按[计划](go-java-v1-raft-commit-timeout-plan.md)，在3000ms选举窗口中完成原AT/TCC业务自然16、两领导更替、真实确认成功丢回复、AT清Undo与原TCC持久重放，最终三副本清理；效果保持1。运行87 Python＋额外报告到副本context绑定4=91，948输入＋3最终校验输入、341classpath/2821文件、六snapshot/两加载/两JFR/20类来源/五原生查询30帧闭合。Go254/Rust117/Java RAFT52继承未重跑。

1000ms第二次选举分票失败继续OPEN，不能以3000ms成功替代。原11==13上下文，未认证Prepare后更改context复制；原Java仅复制status的静态风险及事实/推断区分见[运行时发现](raft-commit-runtime-findings.md)。原参考问题、指标、通用Fence、全故障矩阵、生产认证、正式Rust和完整WP00仍开放。


## RAFT Prepare 上下文更新修复候选（2026-11-13）

[计划](java-raft-context-replication-candidate-plan.md)对应[原实现真实缺陷](evidence/local-go-java-raft-context-reference/README.md)与[0005候选064040](evidence/local-java-raft-context-candidate/README.md)。原11/13确实不同，原副本保留旧上下文，选主后实际TCC拒绝确认、业务效果0；负向运行不计恢复成功。候选统一补丁组三节点在两次领导更替、六snapshot/两原盘加载、两次DB/SDK重启及真实成功回复丢失后，同原分支持久重放确认仍10/效果1，最终三副本清理，余额900/Undo空。

直接绑定原13/14→12副本观察→全部真实TCC回调（阶段5/1/1）→五SQL准备token及两个接受记录；5TC/3Go SDK、35实际类来源、两JFR重读和五原生查询30帧完整核验。Java同字节25测试原类RED11失败/14控制、候选25通过，真实原类705B旧消息兼容通过。Python102＋追加实际证据破坏10＝112；归一化15已包含，空guard适配器RED仅作校验器负控，不冒充原生产RED。1039运行输入＋10追加验证输入、341classpath/2821依赖、23生产覆盖类/1只读控制/3辅助类前后闭合；资源停止、11端口释放。Go254/Rust117及旧Java父范围继承未重跑。

JAVA-RAFT-BRANCH-CONTEXT-001原参考仍OPEN，独立0005取得选定范围证据；只认证统一补丁与新reader读旧消息，不支持混合版本滚升声明或修复历史已丢上下文。1000ms分票、metadata/logging、指标/Exporter、全故障矩阵/通用Fence/认证、正式Rust、完整WP00和Phase1–7继续开放；分阶段冻结提案仍待确认。


## Java / Go AT 共享锁实际缺陷（2026-11-13）

[计划](java-go-at-shared-lock-reference-plan.md)对应[070224负向主证据](evidence/local-java-go-at-shared-lock/README.md)。真实Java与Go AT在同一MySQL行同时保留已提交阶段一，余额1000→900→800、两个原Undo并存；固定Java锁键仅换resource即可看不到原锁，资源命名空间隔离由实际查询证明，锁键文本差异仍单独开放。GO-AT-RESOURCE-ID-001由源码推断升级为所选组合实际锁绕过，未认证安全互斥或迁移。

32 Python=归一化12＋实际证据破坏20；后者旧冻结校验器RED18失败/2控制、新门禁20通过。118运行/32追加输入、342classpath/2802依赖、16类来源、6原关键源/类、12原生查询72帧及真实TM/报告/原Undo分别闭合。Java客户端默认V2、Go V1原样记录，未扩张V1认证边界。原TM提交清理后余额800/Undo空/锁释放，3自有进程及容器退出、四端口释放。Go254/Rust117单元继承无新执行；正式Rust、WP00和全计划其余门禁仍未完成。


## 全局状态语义运行验证增量（2026-11-13）

依据[独立计划](java-global-status-contract-plan.md)，[正式证据](evidence/local-global-status-contract/README.md)补齐原Java21状态/189辅助分类、441校验对（108拒绝），四种响应168个成功/失败样例在原Go及既有显式候选保持字段/字节；原Go保留940未知状态响应的数字，显式入口全部拒绝，Java235未知枚举拒绝。22项新门禁全通过，28输入/278classpath/1950依赖/16认证原JAR类/3执行产物闭合，92文件封存并经只读复审。原Go16–20缺少具名语义，GO-V1-STATE-001、业务调度/转换可达性、WP00和生产Rust继续开放；共享锁仍按用户要求延期。
