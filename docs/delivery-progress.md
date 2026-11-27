# Seata-Rust 全计划交付跟踪

目标：完成用户引用的 Seata-Rust 全部计划内容。本文仅汇总状态与证据入口，不替代规格、不缩减范围，不把测试探针计作生产组件。

规格事实源：[架构与 Phase 0–7 路线](architecture/SEATA_RUST_ARCHITECTURE_PLAN.md)、[V1 WP-00–07 验收清单](phase-0/v1-implementation-acceptance-checklist.md)、[Phase 0 集成计划](phase-0/java-tc-live-integration-plan.md)、各 ADR。未初始化 Spec Kit/OpenSpec，普通 Markdown 计划不代表已安装或初始化对应体系。

| 阶段 | 原计划交付范围 | 当前证据与差距 | 状态 |
|---|---|---|---|
| Phase 0 / WP-00 | Java/Go 源码映射、RSeata 审计、协议/状态对照、ADR、兼容性规范及运行时基线冻结 | 见 [ledger](phase-0/validation-ledger.md)；冻结29样例及Java处理器78输出已比较；响应丢失、FILE/DB/Redis恢复、认证/双资源重连及真实RAFT六次切换有证据；新增控制帧、24文本边界及250异常码响应；Java475/Rust117/Go235/新Python9通过；新增合并载荷及有符号/可选语义候选，原版默认/传输仍未修复；三个心跳三方一致，原版六个合并帧Go缺codec，显式载荷候选已通过，显式SDK会话/重连/分发已有选定路径证据，Java首字节分片仍失败；Go差异、FILE/RAFT恢复异常及其余门禁仍开放 | 进行中 |
| Phase 1 / WP-01–03 | 正式协议模型/编解码、连接/关联、TM、XID/异步上下文及传播、互操作 | 正式Cargo workspace、协议/传输/TM已通过获批首期各组件Gate；全部V1与后续兼容性门槛继续开放 | 首期组件已验证 |
| Phase 2 / WP-04–06 | 正式 RM、SQL 拦截、镜像、锁、Undo Log、MySQL AT 原子提交/补偿与幂等 | 正式RM拥有登记池/单次出站/原连接批回调/持久日志及强杀恢复；45RM纳入234项双版本回归。WP04首期十项及真实Java六例Gate现已通过；SQL/Undo继续WP05/06，模拟TC和文件资源夹具不证明数据库原子性 | 进行中 |
| Phase 3 / WP-07 | 生产配置、可观测性、认证/TLS、优雅停机、兼容矩阵、故障/并发/压力/安全审查及操作指南 | 单次探针不是生产就绪验收 | 未完成 |
| Phase 4 | XA MySQL、两阶段提交、TCC Resource/Fence、空回滚与悬挂 | Java XA 身份与 MySQL 原生两阶段已有双版本/真实重启证据；完整 XA Resource/TC 门禁、TCC/Fence/空回滚/悬挂继续 | 进行中 |
| Phase 5 | Rust TC：Global/Branch Session、状态机、锁、存储、恢复、超时/重试；Java/Go/Rust 兼容 | Java TC 验证不能替代 Rust TC，尚无正式实现 | 未完成 |
| Phase 6 | 集群/故障切换、注册发现、配置中心、多存储、管理 API、监控审计 | 需按 Phase 5 稳定边界实施与验证 | 未完成 |
| Phase 7 | Saga、PostgreSQL、SeaORM/SQLx/Diesel、更多 RPC、CLI、扩展插件及 Java 完整能力对照 | 原架构各能力矩阵均纳入目标；不得以目录/feature 声明作为完成证据 | 未完成 |

跨阶段硬门槛：架构 §9 跨语言矩阵、全部故障场景、SQL 支持/拒绝矩阵、§11 V1 DoD，以及 WP 清单每个编号项均需可定位的源码、测试和运行证据。没有证据的项目维持未完成；性能目标、适配器与公开契约在正式变更中具体化，不静默删除或改写。

当前执行顺序：按已批准的protocol-first-v2，从已通过的WP00-P及WP01–04继续WP05–07，再完成Phase4–7；WP00-R及外部TC风险继续开放。未授权事项（初始化/安装、切换分支、提交/推送、覆盖已有配置等）不因长期目标自动获得授权。

早期调查队列（其后状态修订见文末，当前实施已推进至WP04）：

1. 冻结 29 样例的三方验证、补充 7 个失败对照及二进制溯源已完成；后续需处理 [GO-V1-FAIL-001–003](phase-0/known-go-v1-failure-divergences.md) 的真实差异，新增[心跳/合并基线](phase-0/protocol-v1-control-envelopes.md)：三个心跳三方一致，六个Java/Rust合并样例在固定Go缺少59/60 codec（GO-V1-MERGE-001）。Java163/Rust114/Python5及静态检查通过，[归档](phase-0/evidence/local-control-envelopes/README.md)。type14/22现有独立长度候选，新增[显式异常响应入口](phase-0/evidence/local-go-java-exception-profile/README.md)覆盖25码×10响应、250指针检查和原七个失败语义；[合并载荷候选](phase-0/evidence/local-go-java-merge-profile/README.md)新增38组合/边界与原六例验证；Java475/Rust117/Go110通过，显式帧入口与自定义TCP往返已有补充证据，但已新增SDK会话选定路径证据，原版缺口及完整业务客户端仍开放；完整Go兼容目标不缩减，Rust仍是测试工具。
2. 八类 handler／鉴权钩子测试及所有78个实际可编码输出的 Go/Rust 比较已完成。GO-V1-STATE-001 与 JAVA-V1-REPORT-NULL-001 仍需明确业务/API策略和验证；比较已执行不等于相关问题已修复。
3. INT-013–014、MySQL/Redis八个恢复场景、认证钩子16次拒绝和双资源重连已有相应范围证据。真实RAFT003–004诊断新增三组集群、六次切换、三次快照重载；16/17原记录/锁保留且控制台/原生可见，但四次原异步恢复异常仍开放：[证据](phase-0/evidence/local-raft-retry-timeout/README.md)。Java新鲜4项、Rust106项、Python3项通过。继续明确FILE/RAFT外部限制下的兼容性边界、自然超时及008–009真实补偿/持久重放，再冻结WP-00运行基线。指标、凭据日志、RAFT元数据/日志兼容性问题及生产认证仍未交付，不能因诊断脚本成功而关闭。
4. 已新增[WP-00冻结前核对](phase-0/wp00-freeze-readiness-audit.md)，逐项区分已执行调查、未通过验收与正式模块依赖。继续解决差异/适配方案及冻结门禁；[分阶段冻结提案](phase-0/wp00-staged-freeze-proposal.md)已准备，等待确认后才按调整后的依赖推进正式WP-01组件。后续 Phase 1–7 不因当前子项完成而从目标中移除。

完成审计必须逐条回到上述规格与矩阵，确认所有明确要求和交付物有相应证据。单阶段通过、编译通过或部分测试为绿，均不足以将总体目标标记完成。

## Getty 显式帧候选增量（2026-11-12）

0004通过实际帧处理器326例（Java完整帧9例、工具封装认证载荷317例）、未改动Java TC注册/合并/两原始事务回滚的8请求/8响应；新鲜Java475/Rust117/Go181、新Python6及继承24项Python回归通过。审查发现的源码模板/执行补丁绑定缺口已修复，前一阶段副本与本阶段副本分离。[证据](phase-0/evidence/local-go-java-getty-profile/README.md)。下一步为[原SDK会话、重连与响应分发](phase-0/go-getty-runtime-gap-audit.md)，不将当前自定义TCP探针计作完整SDK。正式生产Rust及Phase1–7仍未完成，待确认的冻结提案不因此自动生效。

## SDK 会话与关联候选增量（2026-11-12）

0005通过原SDK真实Getty会话链路，覆盖17请求/17响应、EOF重建、双连接心跳、重复/晚到及不一致响应；新鲜Java475/Rust117/Go235、13新增Go和20次并发重复、新Python9及继承30项Python回归通过。审查提出的批次所有权、类型化错误传播、响应信封/注册类型及初始化配置问题均有实际失败/修复证据。[证据](phase-0/evidence/local-go-java-sdk-profile/README.md)。

该历史SDK场景未覆盖TC方向任意分片；后续独立Java候选的双向分片结果见下节，不能回填为原制品已兼容。真实RM/Undo、生产Rust、WP-00和Phase1–7仍未完成，既有门禁及待确认的阶段调整保持有效。

## Java 前导分片候选增量（2026-11-12）

对应[JAVA-V1-PREFACE-001](phase-0/java-v1-preface-fragmentation.md)与[候选计划](phase-0/java-preface-candidate-plan.md)。原版569项已有回归通过，新48项目标测试29失败/19控制通过；独立候选617项/27套通过，Rust117/Go235/Python7及静态检查通过。[完整证据](phase-0/evidence/local-java-preface-candidate/README.md)。

保留原ProtocolDetector接口，增加可选前缀等待和独立索引视图；Seata等齐版本分发所需3字节，HTTP按实际可读起点识别。全新构建的10个class独立覆盖原JAR，5个主类实际加载来源已核对；原JAR再次复现首字节拒绝，候选通过双向强制分片的实际SDK17请求/17响应、两连接、故障注入及两个原XID回滚，零事务重发。HTTP仅验证404/SETTINGS的协议选择。

原Java源码与JAR、既有Go副本保持不变，候选未合入发布；V0继承分片问题及V2专门认证、SDK其余在途故障/完整业务客户端、真实RM/Undo和生产Rust仍开放。下一步继续这些兼容性缺口及FILE/RAFT恢复边界；WP-00冻结提案仍待确认，正式Phase1–7未完成。

## SDK 在途结果候选增量（2026-11-12）

对应[在途验收计划](phase-0/go-java-v1-inflight-plan.md)及已接受ADR-0008。0006只在显式同步路径区分未发送与结果未知，绑定真实会话/确切future，保留TC明确响应；完整Go254（235继承+19新增）、Rust117/Python5及静态检查通过，Java617明确继承已认证父证据，本轮未重跑。

原0005同一19项行为对照、处理器暂停/ID复用、binder查找窗口和并发代理分片均保留失败证据。统一短锁及入站对象快照修复归属竞态，合法TC→RM同ID请求不误丢弃；旧子项批次新增成功/整批未知/新所有者保护控制。代理整帧分片按连接/方向加锁。

最终实际SDK四连接均注册，19请求/19实际TC响应，Begin4/Commit1/Rollback3、查询7；三次响应丢失断连和一次自然20秒超时均返回可识别UnknownOutcome，晚到原响应不复活等待。Commit状态实际8→15，只查询不重发事务，四原XID最终核对/清理。[归档证据](phase-0/evidence/local-go-java-inflight/README.md)。

原版/default及前五补丁、旧副本与原Java JAR保持不变。未知Begin的XID来自测试TC观察者，不代表通用Begin幂等恢复；完整业务/异步客户端、自动聚合发送策略、真实RM/Undo、TLS、生产Rust、完整WP-00与Phase1–7仍未完成，分阶段冻结提案仍待确认。下一步继续完整客户端初始化/资源注册与自动聚合，以及FILE/RAFT恢复和真实补偿缺口，不重复把选定同步链路计为全部交付。

## 完整Go SDK与真实MySQL AT增量（2026-11-13）

按[真实AT计划](phase-0/go-java-v1-real-at-plan.md)完成三场景新鲜验证：原client.InitPath/TM/RM/SQL代理正常提交和补偿；阶段一后断连及真实资源重注册，重复回滚生成status1防悬挂记录且业务不再改变；原SDK进程退出、MySQL重启后原XID/BranchId/完整Undo字节不变，新PID补偿原事务且不发第二Begin。TC终态查询和独立业务/Undo SQL分别核对。[完整证据](phase-0/evidence/local-go-java-real-at/README.md)。

Python16通过，759输入导入前冻结，Java覆盖类固定清单/10class字节链及每场景前后/实际加载来源核对。原Go714成员及既有24候选/13辅助未改，Go254/Rust117/Java617明确继承已认证父证据，本轮未重跑其单元测试。审查发现的恢复代理/关联、空SQL快照和class哈希缺口经真实负向测试后修复；期望的prepare断连单独分类，其他错误不放过。

这些是参考Go真实资源证据，不是生产Rust。正常Begin/Registered持久重放不能替代RetryTimeout16/17的008–009完整验收；全SQL/故障原子性矩阵、混合语言共享资源、生产认证及完整WP-00/Phase1–7仍开放。新记录[GO-AT-RESOURCE-ID-001](phase-0/go-at-resource-identity-divergence.md)：原Go标识含DSN凭据且命名不同，不能据单Go RM成功推断Java/Go/Rust共享锁安全。后续推进该边界及真实重试超时恢复，既有冻结提案仍待确认。


## 真实AT自然重试超时与持久重放增量（2026-11-13）

依据[真实AT超时计划](phase-0/go-java-v1-real-at-timeout-plan.md)完成新的MySQL DB存储TC联合运行：完整Go SDK只创建一次原事务和真实SQL/Undo分支，代理阻断实际回滚指令，原调度器自然写入RollbackRetryTimeout17；原TC/SDK进程退出及MySQL重启后，原XID/BranchId/资源/锁与完整Undo字节保留。实际控制台人工重试由真实RM补偿余额900→1000并删除Undo，成功响应被代理丢弃，TC仍保持17及原分支/锁；再次TC/SDK/MySQL重启后再人工重试原分支，余额保持1000，TC记录/分支/锁清理，Go RM写入同原身份status1防悬挂Undo。[正式证据](phase-0/evidence/local-go-java-real-at-timeout/README.md)。

三个不同TC和完整SDK进程、两次数据库重启、实际注册/分支/回调连接与请求ID、五阶段SQL/控制台及自然17日志分别核对。首次HTTP500必须与原分支响应超时对应，最终成功不只依赖HTTP200。本轮新执行Python25通过，包含保持帧哈希/序号合法的原始证据负向用例；Go254/Rust117/Java617明确继承父证据，本轮未重跑。770项输入导入前冻结；固定JDBC8.0.33在每阶段前后校验字节与实际class来源，原Java JAR与Go候选副本未改。失败收尾尝试及修复后完整重跑均保留，正式运行容器/进程已停止、端口已释放。

REC-TIMEOUT-009的真实MySQL AT补偿/丢响应/持久重放取得该链路证据，仍非所有SQL、并发崩溃和存储组合验收。REC-TIMEOUT-008/自然16仍开放：固定Java纯AT通常走AsyncCommitting，不能靠改状态或短时观察声称自然16。FILE/RAFT缺陷、资源标识差异、生产认证、正式Rust、完整WP00和Phase1–7仍未完成；既有分阶段冻结提案仍待确认。下一步补真实提交重试及剩余恢复边界，不把选定参考链路当成生产交付。


## 真实AT＋TCC自然提交超时与持久重放增量（2026-11-13）

按[真实混合提交计划](phase-0/go-java-v1-real-commit-timeout-plan.md)完成新鲜运行：完整Go SDK建立原AT/TCC两分支，真实AT业务余额1000→900并生成Undo，TCC业务真实预留10。仅阻断真实TC Commit指令，让原调度器自然持久化CommitRetryTimeout16。TC/SDK/MySQL重启后保留原XID、两BranchId、资源、AT Undo完整字节和TCC预留。人工重试时原AT实际清理Undo并答复，原TCC本地确认10后成功响应丢失，TC维持16且只剩原TCC分支；再次TC/SDK/MySQL重启后人工重试原TCC，确认仍10、效果计数仍1，最终TC global/branch/lock和Undo均空，AT余额保持900。[正式归档](phase-0/evidence/local-go-java-real-commit-timeout/README.md)。

原TM仅一次Begin/Commit、无Rollback或目标重发。首次operator为HTTP200/body code500，须匹配原TCC等待响应超时及AT提交成功日志，不能当成完全成功；最终HTTP200/code200同时经过帧、独立SQL及控制台验证。每个callback与原分支类型/资源/连接/请求ID及唯一成功回复绑定。审查发现的重复TCC成功替代AT回复、错误envelope、SQL冻结遗漏已修复；合法不同V1 header负向用例有修前RED和修后GREEN。

本轮Python27新执行通过；Go254/Rust117/Java617明确继承，未重跑。783输入导入前冻结，含新增schema.sql；固定JDBC8.0.33/原JAR、每轮实际Driver类来源及10个覆盖类字节/5主类来源前后核对。自有容器/进程已停止，端口释放。原TC重试指标订阅者的null endTime异常再次出现，JAVA-METRICS-RETRY-001保持OPEN，见归档事件；未将业务恢复通过冒充指标已修复。

这是REC-TIMEOUT-008选定AT＋TCC真实提交恢复与持久业务幂等证据，与此前009真实回滚链路互补。TCC幂等来自本次业务的本地事务/唯一身份/行锁，不表示SDK自动保障任意业务；纯AT AsyncCommitting也不是本次自然16触发源。全SQL/并发崩溃矩阵、完整TCC Fence、FILE/RAFT安全恢复、混合语言资源标识、生产认证、正式Rust、完整WP00及Phase1–7仍未完成。既有分阶段冻结提案仍待确认。下一步处理剩余存储恢复和资源兼容边界，保持全部计划范围。


## Java重试超时保留候选与真实FILE重启增量（2026-11-13）

按[候选恢复计划](phase-0/java-retry-timeout-retention-candidate-plan.md)形成独立Java0002补丁：FileSessionManager不再过滤17，SessionHolder显式保留16/17且不激活、不改成自动重试；16不重新获取提交锁，17按原两个解锁开关决定保留回滚锁。DefaultCoordinator两个既有超时入口和恢复共用原启动快照，避免热变更provider产生生命周期分叉。未修改原Java源码/JAR、旧前导补丁或正式Rust策略。[完整证据](phase-0/evidence/local-java-retry-timeout-candidate/README.md)。

最终Maven72项新鲜通过（37目标＋35受影响原测试），Python12项通过。相同编译测试字节对原JAR有23失败/14控制，对实际部署覆盖类37全过；独立JVM曾复现动态配置读取12失败/25控制，共享快照后通过。真实FILE六情形、12个完整TC进程：Begin控制保留锁；16成功启动并保留原会话/分支、提交锁释放；17保留原身份并遵守两flags四种组合。控制台与仅101/15/21原生查询分别核对，无Begin/End或RM资源注册。原2122生产文件仅三处候选差异，29仓库输入导入前冻结、两外部元数据及340classpath/2800实际依赖绑定，19覆盖类字节/三个候选实际来源核对。自有TC停止、28091释放。

旧夹具引导、缺extraData、误判类加载和编译元数据缺口等失败/较弱运行保留，不计正式通过。原未打补丁FILE16/17问题仍存在；候选只取得本次seeded真实FILE及单元范围证据，不冒称自然超时/真实RM补偿。RAFT分支单元覆盖不能代替实际选举/快照；原未观察CompletableFuture的失败安全性仍需专门验证。指标、RAFT元数据/日志兼容、混合语言资源标识、正式Rust、完整WP00与Phase1–7仍开放；分阶段冻结提案仍待确认。后续继续候选的真实RAFT重载与剩余资源恢复，不降低原验收。


## Java RAFT 候选真实选举与恢复完成观察增量（2026-11-13）

按[观察候选计划](phase-0/java-raft-reload-observation-candidate-plan.md)新增独立0003补丁，组合既有0001/0002；原Java源码/JAR、旧补丁和正式Rust策略保持原样。实际SessionHolder.reload异步成功才记录捕获的任期，单调更新避免旧任务晚完成覆盖新成功；失败记录分组/任期/异常。标记仅供观察，不改变isLeader，不代表服务就绪或失败领导隔离。[完整证据](phase-0/evidence/local-java-raft-reload-candidate/README.md)。

真实三节点覆盖Begin1对照、CommitRetryTimeout16及默认双解锁开关关闭的RollbackRetryTimeout17：6次领导切换、3次快照重载、12个独立TC JVM。16保留原身份/分支/状态且清理残锁；17保留原记录和锁，不转自动重试。18组当前领导确认核对Seata/JRaft实际前后任期及领导身份、第二次取样、复制位置和原记录。18次只读101/15/21查询的108原帧重新解码并核对完整请求；48条实际主类来源及6份JFR原件核验。

本轮新执行Java52项（8目标＋44上游RAFT原回归）、Python18项通过。同一目标二进制对原JAR为6失败/2控制，对实际运行覆盖类8通过；44项以正确Spring环境、实际覆盖类和编译依赖前后绑定执行。轻量环境回归失败及早期较弱运行保留作诊断，不计最终通过。53输入导入前冻结，父清单固定认证19类，340classpath/2800文件继承并核对；21覆盖类和实际JFR reader等生成类前后守卫。最终完整性失败先转失败再写结果。自有进程停止、9端口释放。

本候选取得seeded真实RAFT选举/快照恢复及异步成功观察证据；不是自然超时、真实RM补偿、额外解锁配置实测或失败领导隔离验收。原版FILE/RAFT、元数据/日志、指标、混合资源标识、正式Rust、完整WP00及Phase1–7仍未完成；分阶段冻结提案仍待确认。下一步验证自然超时后的FILE业务恢复与其余存储/资源边界，继续全部计划。


## FILE自然17与真实AT持久补偿增量（2026-11-13）

依据[FILE业务恢复计划](phase-0/go-java-v1-file-at-timeout-plan.md)，完成独立19类Java候选的真实FILE链路：完整固定Go SDK只创建一次原事务/SQL/Undo并发起一次Rollback；代理阻断8个真实回滚请求，原调度器自然进入17。实际FILE journal经停止及新TC/SDK/MySQL重启后，原XID、BranchId、资源、回滚锁与完整Undo字节保留。人工操作由真实RM补偿900→1000并删除Undo，真实成功响应丢失后TC仍17及原分支/锁；再次停止重启后人工重放原分支，业务仍1000，原TC会话/分支/锁移除，Go持久化同原身份status1防悬挂Undo。[正式证据](phase-0/evidence/local-go-java-file-at-timeout/README.md)。

MySQL仅accounts/undo_log，TC实际session/lock均FILE。3TC/3SDK与两次MySQL重启，日志/配置/PID/类来源及停机journal→下一轮启动原字节分别绑定；原journal在丢成功响应阶段也未变。五阶段独立SQL、控制台、只读原生5查询/30帧及SDK注册/回调/请求ID/方向/外层类型核对；原Begin回复及Rollback目标XID均绑定。首次HTTP500对应原分支响应超时，最终HTTP200/code200还须业务与TC清理证据。

本轮Python44项新执行通过（13归一化、8FILE、23真实证据破坏控制），含先前7/9/1项行为RED的修复。803输入导入前冻结，Go cohort及原JAR、SDK binary、配置、19覆盖类字节前后一致，24条实际主类来源核验；完整性检查先于成功写入。Go254/Rust117/Java617明确继承未重跑。首轮较弱校验运行保留诊断；自有TC/SDK/代理和容器停止，3端口释放。

REC-TIMEOUT-009取得本候选选定FILE自然17与真实业务持久重放证据；其他SQL/并发/崩溃、FILE自然16、RAFT真实业务、额外配置和生产策略继续按原范围验证。原版FILE/RAFT缺陷、指标/元数据/日志、混合资源标识、正式Rust、完整WP00及Phase1–7仍未完成，分阶段冻结提案仍待确认。下一步继续FILE自然提交超时和其余运行时边界，保持全部计划目标。


## FILE自然16与真实AT/TCC持久提交增量（2026-11-13）

依据[FILE混合提交计划](phase-0/go-java-v1-file-commit-timeout-plan.md)完成独立19类Java候选的真实自然16链路。完整Go SDK只发一次Begin/Commit、零Rollback；真实AT本地SQL/Undo与TCC预留后，代理阻断9个原Commit请求，原调度器自然进入16。TC/SDK及MySQL重启后原XID、两分支、AT完整Undo、TCC预留保留且提交锁可用。实际人工重试中原TCC确认10后成功回复丢失，原AT真实清理Undo并答复；TC仍16且仅原TCC。再次重启并人工重放原TCC后，余额仍900、Undo空、确认仍10且效果计数仍1，原TC会话/分支清理。[正式证据](phase-0/evidence/local-go-java-file-commit-timeout/README.md)。

MySQL仅四个业务/Undo/TCC表，TC实际session/lock为同步FILE。3TC/3SDK及两次MySQL重启，两次启动journal分别精确等于前次停机字节；部分提交移除AT与最终清理后journal正常追加。原13报告、成功14确认、保留分支状态/应用上下文和实际Commit回调逐项绑定，确认必须先于首次回调；原Begin/Commit目标、注册、方向、外层类型、连接/请求ID、真实丢帧和operator顺序均核对。五阶段独立SQL/控制台/5原生查询30帧，以及24条实际主类来源分别重算。

本轮Python57项新执行通过（13归一化、8FILE、36真实证据破坏控制），包含先前7/3/2项行为RED的修复。819输入导入前冻结，父783及共享代码/候选类认证；Go cohort、原JAR/SDK binary/配置/19类字节前后匹配，完整性检查先于成功写入。Go254/Rust117/Java617明确继承未重跑。早期两次较弱门禁运行保留诊断；自有资源停止，3端口释放。

JAVA-METRICS-RETRY-001本轮再次出现2条原指标订阅者null endTime异常，独立归档并保持OPEN，未将业务恢复通过冒充指标修复。REC-TIMEOUT-008取得本候选选定FILE自然16与持久业务幂等证据；完整TCCFence、RAFT真实业务、其他SQL/配置/并发/崩溃矩阵仍单独验收。原参考缺陷、元数据/日志、资源标识、生产认证、正式Rust、完整WP00及Phase1–7仍未完成，分阶段冻结提案仍待确认。下一步处理已复现指标异常与其他剩余边界，继续全部计划。


## Java指标生命周期候选与真实FILE16增量（2026-11-13）

依据[指标候选计划](phase-0/java-metrics-lifecycle-candidate-plan.md)新增独立0004补丁：producer的doing事件有意保留null endTime，完成状态消费者须先区分未完成事件，避免重入16/17时误扣active、伪增失败并抛NPE。Begin/三Retrying保留原规则；有效done指标名称/标签/计数不改，缺begin/倒序/溢出仅跳Timer，不补零或当前时间。原JAR、producer及事件未修改。[正式证据](phase-0/evidence/local-java-metrics-lifecycle-candidate/README.md)。

同一实际CompactRegistry测试字节对原JAR为47失败/30控制，候选77项通过（65目标＋12固定pin原回归）；4源先冻结再重编，测试/候选class与实际部署字节一致，278classpath/1950文件前后绑定。新执行Python72项：父业务57重跑、指标破坏15；异步完成标记曾2失败/13控制，修后全绿。Go254/Rust117/Java617完整父范围明确继承，不与77直接相加。

真实FILE16混合业务继续通过3TC/3SDK与两次journal/MySQL恢复、真实AT清Undo、TCC成功丢回复和持久重放。每进程SDK前基线独立全0；初始自然超时failed/timeout/timer各1且5486ms；两个恢复进程实际null-end doing被精确DEBUG标记确认后采样，重入不增完成计数、active保持0；最终真实AfterCommitted counter/summary/timer各1，实际3ms。8快照/8回执、原总线事件、30类来源及原SQL/控制台/5查询30原帧分别核对。只读get/count/total不measure/clear；Exporter仅绑独立端口，未scrape，不据此验收Exporter输出。

834仓库输入导入前冻结、40外部输入（含源码快照/类/编译参数/日志/D冻结脚本）及运行依赖完整性通过后才写成功。MySQL临时套接字误ready的失败及缺显式顺序屏障的早期运行保留诊断；正式采用TCP就绪和“同TC/原transactionId跳过标记→采样”门禁。自有资源停止，五端口释放。

JAVA-METRICS-RETRY-001原参考仍OPEN，0004在本候选FILE16事件生命周期范围取得通过证据；不等于全部重启计数重建/去重、Exporter、真实FILE17/RAFT指标或完整生产验收。其他SQL/配置/并发/崩溃、资源标识、认证、正式Rust、完整WP00与Phase1–7继续开放，分阶段冻结提案仍待确认。后续继续真实回滚指标及其余运行时门禁，保持全部计划目标。


## Java指标候选与真实FILE17回滚增量（2026-11-13）

依据[回滚指标计划](phase-0/java-metrics-rollback-candidate-plan.md)，同0004生产字节接入真实FILE自然17。原完整Go SDK一次Begin/Rollback、原调度器自然超时，TC/SDK及MySQL两次重启并读取前次停机相同journal；第一次实际补偿使余额1000且Undo空，成功回复丢失后TC保留原17/分支/锁；再次同分支重放后余额仍1000、防悬挂Undo为1/{}，原TC会话/分支清理。[正式证据](phase-0/evidence/local-java-metrics-rollback-candidate/README.md)。

3个TC启动前各自零基线；初始failed/timeout/timer各1=5485ms。两个恢复进程实际nullable17 doing事件经精确DEBUG处理标记后采样，未误扣active或重复增加失败；最终真实AfterRollbacked counter/summary/timer各1=3ms。8快照/8回执、原事件、30实际主类来源、独立SQL/控制台/5原生查询30帧、两次journal恢复输入均复核；只读观察不measure/clear，未抓取Exporter。

本轮新执行Python71项（业务44、回滚样例12、真实指标破坏15）。样例先用旧提交校验器得到3失败/9控制，再实现回滚检查通过；正式证据由真实运行推导。Java77继承未重跑，本轮重编MetricsSubscriber与已测试class完全相同；Go254/Rust117/Java617完整父范围也明确继承。854仓库/40外部输入、278classpath/1950文件及原SDK binary/Java JAR/Driver/配置/候选类前后匹配，自有资源停止，五端口释放。

JAVA-METRICS-RETRY-001原参考保持OPEN，0004取得选定FILE16和FILE17生命周期证据；不含并发组合、全重启指标重建/去重、Exporter、RAFT真实业务/指标或全部SQL/配置/崩溃矩阵。资源标识、认证、正式Rust、完整WP00与Phase1–7仍开放，分阶段冻结提案仍待确认；后续继续其余运行时门禁。


## 真实RAFT自然17与持久AT恢复增量（2026-11-13）

依据[RAFT AT计划](phase-0/go-java-v1-raft-at-timeout-plan.md)，已完成[真实RAFT自然17证据](phase-0/evidence/local-go-java-raft-at-timeout/README.md)：原Go SDK一次Begin/Rollback、真实SQL/Undo，原TC自然超时；三节点snapshot后停止原领导，实际补偿至1000/Undo空但丢原成功回复，TC仍保留17/原分支/锁；恢复原节点、再次停止领导后同分支持久重放，余额仍1000，仅防悬挂Undo=1/{}，最终三个副本清理会话/分支/锁。

实际5个TC JVM/3个Go SDK进程、两次业务DB重启、两次可见领导更替（任期1→2→4）、六snapshot/两次原盘精确index加载；每轮当前Seata/JRaft任期/peer/reload成功、全副本进度及重复确认，业务路由/人工恢复/查询前后同任期绑定。真实protobuf元数据重读、两份JFR重新解析与停止窗口、20类来源、五原生查询30帧及原RPC/SQL/Undo/控制台完整复核。

运行65 Python（13业务、19原始业务、21集群、3输出、9完整副本），最终额外停止身份4测试合计69；额外门禁绑定回执与前任确认领导，源码/测试/结果3输入独立冻结，单独runtime succeeded不能替代最终认证。923运行输入、341classpath/2821文件和原JAR/SDK/Driver/配置及20生产class前后相同；Go254/Rust117/Java RAFT52明确继承未重跑。诊断失败与清理变量问题均保留，正式运行资源停止、11端口释放。

JAVA-RAFT-RELOAD-001原参考仍OPEN，选定候选真实自然17恢复取得通过；metadata/logging原问题保持OPEN，本轮18处PutNodeMetadataResponse字符串非独立事件数。未验收RAFT指标/Exporter、自然16、全SQL/分区/并发/损坏/崩溃或生产认证。资源标识、正式Rust、完整WP00与Phase1–7继续开放，分阶段冻结提案仍待确认。


## 真实RAFT自然16混合提交增量（2026-11-13）

依据[RAFT提交计划](phase-0/go-java-v1-raft-commit-timeout-plan.md)，[主运行055235](phase-0/evidence/local-go-java-raft-commit-timeout/README.md)在明确3000ms选举窗口下完成原Go AT/TCC混合业务自然16。原TM一次Begin/Commit、零Rollback；两次实际领导更替与DB/SDK重启，真实TCC确认10成功回复丢失、AT真实清Undo后TC仅保留原TCC；同分支持久重放后确认仍10/效果仍1、余额仍900/Undo空，最终三个副本会话/分支/锁清理。

实际5TC JVM/3Go SDK、六snapshot/两次原盘加载、两份JFR重新读取、20关键类来源及五原生只读查询30帧，当前任期/peer/成功reload/副本进度和全部业务观察前后同任期核对。原13/14、分支状态/上下文与实际回调、注册/身份/方向/相关ID完整复核；停止身份门禁本轮已纳冻结runtime。运行87 Python及额外原报告到17条副本分支上下文绑定4，共91新执行。948运行输入＋3最终校验输入、341classpath/2821依赖、同20生产class/同3只读诊断class和原JAR/SDK/Driver/配置前后认证，资源停止，11端口释放。Go254/Rust117/Java RAFT52继承未重跑。

1000ms配置054700在第二次选举多轮分票、60秒未出现可接受领导，保留原日志/配置/投票原行SHA，状态OPEN；3000ms成功不关闭该现象。连接阻塞与选举窗口同量级造成分票仍为推断。原11==13应用上下文字节关系明确，本例Prepare不修改上下文，不能认证Prepare后改写context的复制；固定Java仅复制branch status为静态来源风险，未在本例活体复现，见[运行时发现](phase-0/raft-commit-runtime-findings.md)。原reload/metadata/logging、指标/Exporter、通用Fence、全SQL/分区/并发/损坏/崩溃、生产认证、正式Rust、完整WP00及Phase1–7继续开放。


## RAFT Prepare 上下文真实缺陷与0005修复候选（2026-11-13）

依[增量计划](phase-0/java-raft-context-replication-candidate-plan.md)，先完成[原实现活体复现](phase-0/evidence/local-go-java-raft-context-reference/README.md)：真实Prepare生成并报告新token，原主节点持有新值而两个副本仍旧值；实际选主后原回调缺值，业务拒绝确认，确认/效果0。该运行只证明缺陷，原参考问题JAVA-RAFT-BRANCH-CONTEXT-001保持OPEN。

[0005候选主运行064040](phase-0/evidence/local-java-raft-context-candidate/README.md)以同一业务二进制和统一三节点补丁完成两次实际选主、六snapshot/两原盘加载、两次DB/SDK重启、真实成功回执丢失及持久重放。原13/14更新context直接绑定12副本观察、所有TCC回调（阶段帧数5/1/1）、五SQL token和两实际接受记录。最终确认10/效果1、余额900/Undo空且三副本清理；5TC/3Go SDK、35关键类来源、两JFR与五原生查询30帧分别核验。

新Java25：同测试class对原类11失败/14控制，候选25通过；真实原类生成的705B旧status-only消息控制通过。Python102运行＋10实际证据破坏＝112，15归一化guard已含在102中；其RED14/1为空guard适配器控制，不是旧生产RED。1039冻结输入、341classpath/2821依赖、同23生产类＋1只读控制与3辅助类闭合；资源停止、11端口释放。Go254/Rust117及旧Java父测试明确继承。

只认证统一补丁和新reader读旧消息，不认证混合滚升或历史丢失数据修复；1000ms分票和其他原参考/指标/全故障矩阵/认证仍开放。正式Rust、完整WP00与Phase1–7未完成，冻结提案仍待确认；继续其余既定门禁。


## Java / Go AT 真实共享行锁缺口（2026-11-13）

按[共享锁调查计划](phase-0/java-go-at-shared-lock-reference-plan.md)完成[070224负向证据](phase-0/evidence/local-java-go-at-shared-lock/README.md)：真实Java DataSourceProxy先阶段一持全局锁，原Go SDK仍在同一MySQL行完成独立阶段一，余额1000→900→800且两个原Undo同时可见。保持Java锁键、仅切换资源标识，原锁查询从不可锁变为可锁，独立证明命名空间隔离；实际锁键文本差异也需处理。本项是实际缺陷复现，不是安全互操作通过，GO-AT-RESOURCE-ID-001继续OPEN。

1TC/1Java客户端/1Go SDK；实际Java默认V2与Go V1帧原样保留、分别解析，不扩张Rust V2范围。118运行输入＋32追加输入、342classpath/2802依赖、16类来源/6关键原源类验证、60原SDK帧及12原生查询72帧闭合；Python32本轮执行。原TM提交清理后余额800/Undo空/锁释放，自有资源停止，四端口释放。Go254/Rust117单元继承未重跑。下一步为显式共享资源与锁键兼容方案及拒绝冲突/恢复验证；正式Rust、完整WP00和Phase1–7继续未完成，冻结提案仍待确认。


## 用户指定暂时跳过共享锁问题（2026-11-13）

用户明确要求跳过当前共享锁兼容问题，后续按[延期记录](phase-0/shared-lock-issue-deferral.md)执行。保留已复现缺陷与未完成候选，不继续该问题的实施或验收，不将其标为通过。已启动运行核实结束，自有容器停止，四端口释放。整体目标保持进行中，继续其他可独立执行的计划项。


## 全局状态语义与协议增量（2026-11-13）

已按[状态验证计划](phase-0/java-global-status-contract-plan.md)完成[有界运行与封存](phase-0/evidence/local-global-status-contract/README.md)：原Java全部21状态、九类辅助方法189观察、441校验对（108拒绝）；四种全局响应168个成功/失败样例，原Go和既有显式入口均保持字段/字节。235Java未知枚举拒绝；940未知状态响应变体原Go保留数字，显式入口全部拒绝。

新增22项门禁全通过，缺失检查先失败再修复；复审发现的动态支持闭包、来源逐项绑定与双Probe同改漏检均修正后重新运行，旧诊断记录未改标。28输入/278classpath/1950依赖/16原JAR类/3执行产物来源闭合，92文件封存。未新增生产兼容补丁，未启动TC或数据库。原Go16–20仅数字保持，尚未完成业务状态语义、调度/恢复和转换可达性；GO-V1-STATE-001、完整WP00和Phase1–7继续开放，共享锁问题仍延期。下一步继续其他独立兼容门禁；不恢复已跳过的问题。


## WP-00恢复调查任务闭合（2026-11-13）

已按原任务“运行REC001–010调查或明确记录受阻项”完成[十项逐项核对](phase-0/wp00-recovery-investigation-closure.md)，现行清单只勾选此调查任务。十一历史清单/20,880文件SHA复核，原FILE/RAFT失败、原RAFT未执行人工重试与后续真实业务候选范围分别保留，[只读审计](phase-0/evidence/local-wp00-recovery-investigation-audit/README.md)可追溯。本轮没有新增业务运行或测试、没有修改旧证据。完整恢复/WP00/Phase0和正式Rust仍未完成；依赖调整提案仍待确认，共享锁继续延期。后续继续其余现行门禁，不以本项调查完成放行正式协议实现。


## 原协议候选样例重新验证（2026-11-13）

已完成[原29载荷/6帧当前复核](phase-0/evidence/local-wp00-vector-audit/README.md)：文档25行与所有原夹具逐字节一致，新执行原Go29/6、原Java35断言体及Rust117，Java/Rust有效语义变异精确失败。643冻结输入、475依赖JAR、158原JAR实际类及编译产物核对；没有改变原样例、生产协议或启动服务。现行any-mismatch任务还含补充差异，仍未勾选，完整WP00/Phase0和正式Rust尚未完成；分阶段提案仍待确认，共享锁继续延期。


## 正式协议启动方案已可审查，等待确认（2026-11-13）

[修订提案](phase-0/wp00-staged-freeze-proposal.md)及[具体V2补丁](phase-0/proposals/wp00-protocol-first-v2.patch)完成D盘副本应用检查和只读复审；原任务/Gate逐字保留、WP01–07其他正文不变。显式处理§7：协议证据/allowlist/ADR仍是P启动条件，恢复标准移至R完整退出条件但原要求保留，顺序批准不等于接受TC风险。[首期范围](phase-0/proposals/v1-protocol-allowlist.md)和[证据/测试入口](phase-0/proposals/wp00-protocol-evidence-index.md)已准备。

当前未收到修订版顺序与allowlist的明确批准，[确认状态](phase-0/proposals/approval-status.json)均false，有效清单仍依赖完整WP00，尚未创建正式Cargo workspace。初版遗漏§7前置冲突，已保留并由V2替代，不将旧版确认或“继续”推导为新增批准。正式实现的同一前置确认在连续目标轮次中仍未解决，下一阶段需要用户决定；整体未完成，共享锁继续延期。


## 正式Rust协议组件首批实现（2026-11-13）

用户已明确确认“批准修订版及首期范围”，V2已应用、allowlist已接受，WP00-P五项核对完成。正式[Cargo workspace / seata-protocol](../crates/seata_protocol/README.md)现已建立，原WP01九项中仅勾选域枚举、九组显式码映射、有界大端字段原语三项；未把probe改名充作正式实现。类型化ProtocolError、未知码拒绝、nullable头字符串/普通字段差别、UTF8/长度/预算/写原子性和Debug边界均有实际代码及测试。

[正式基础证据](phase-1/evidence/protocol-foundation/README.md)：12独立测试在Rust1.99/1.85各通过，fmt/Clippy -Dwarnings/rustdoc -Dwarnings通过；根workspace最初影响旧probe成员关系已复现并用exclude修正，旧独立probe117测试及fmt/Clippy新跑通过。650输入/673文件及实际binary归档，未装工具或启动服务。

早期“待确认/无正式workspace”段落保留历史时点，现已被明确批准及实际代码更新。完整WP01、所有消息/帧/连接/TM/RM/MySQL与WP00-R/Phase2–7仍未完成，共享锁继续延期且外部TC风险未接受。下一步实现正式消息模型及载荷编解码，再按原门禁推进帧与三方验证。

## 正式普通消息载荷实现（2026-11-13）

已完成25类普通消息的独立字段对象与PayloadCodec，显式保留Java可选字段、report255/0和注册原始/default版本语义，Failed不受后续状态/lockable影响；预算、完整读取、严格UTF8/码值及Debug边界均有测试。59/60仍属于批准范围，尚未实现，不缩减allowlist。

[普通载荷正式证据](phase-1/evidence/protocol-payload/README.md)：53独立Rust测试在1.99/最低1.85分别通过，原29载荷及425条既有实际Java记录用于正式API字段/字节比对；本轮未新跑Java。fmt/Clippy/rustdoc严格门禁、只读复审及原probe117新回归通过。689输入/721归档文件，catalog SHA256=fe6846e339c5896a8fc9b5ef9b3d8803a081b3c07c76844f21097a082ad422cc。

原nullable任务保持未勾选，因为完整范围仍含59/60及头模型；WP01仍为3/9，完整codec/帧/HeadMap/心跳及全长约束待完成。下一步实现聚合消息，再推进帧与连接；全计划、WP00-R和共享锁延期边界保持不变。

## 正式聚合载荷实现（2026-11-13）

59/60及显式PayloadLimits已实现，批准27类型均具备载荷codec。59有序子消息/完整或省略ID、60结果子类/未设置数组、封闭长度、共享全树计数、最大64聚合层及同buffer长度回填均有实际代码。复审发现预算前扫描数组的问题，新增1个运行RED后修正，完整61测试通过。

[聚合正式证据](phase-1/evidence/protocol-merge/README.md)：61在Rust1.99/最低1.85各通过，六原控制聚合＋38扩展Java聚合字段结构/字节验证；425普通Java记录及原29仍通过，没有新Java执行。fmt/Clippy/rustdoc严格门禁、旧probe117及fmt/Clippy通过。758输入/795成员catalog SHA256=87b8f709aab5e49f36b5ec0c56dd0e4991759a74d907d742d4bba6ceffb0e37d。

WP01消息专用codec任务勾选，现4/9；nullable全范围仍缺头模型，完整帧/HeadMap/心跳/全长及完整向量任务保持未完成。下一步推进正式帧编解码；完整WP01/客户端和所有后续Phase保持进行中，原Go差异/外部TC风险与共享锁延期不因本项完成而关闭。

## 正式完整V1帧与认证语料覆盖（2026-11-13）

已实现Java对象独立文件的ProtocolEncoderV1/DecoderV1、RpcMessage及严格有序nullable HeadMap、空外层心跳、显式帧/头/条目/载荷树预算；保留signed ID和null/empty差别，拒绝长度/未知码/未支持能力/重复或null key。头key哈希前预算顺序的真实RED1已修复。37帧由原Java实际新编码/解码核对，严格来源再次执行；159原class/7关键类/475依赖与9输入闭合，4项Python来源门禁通过。

[最终完整帧证据](phase-1/evidence/protocol-frame/README.md)172217：74独立测试在Rust1.99/最低1.85均通过，fmt/Clippy/rustdoc严格门禁，旧probe117+fmt/Clippy新跑。新37帧与原29/六帧、425记录、44聚合/九控制帧及78独立处理器侧车均核对；1148输入/1192成员catalog SHA256=2993ad4fa4dea37abf3b81a4da4ab698eade391830f59090c39e7f02da800815。Java严格171558 catalog=ebd29a04818c319bb1d8c805e84b25f9ed1ddd2717ceb4133a992e420e6a4b4a；不存在额外37个JUnit测试或本验证脚本内Java重跑的声明。

原WP01九项实施checkbox已完成，但完整Gate继续OPEN，下一步收敛正式peer-profile三方比较/显式Go适配，再推进WP02连接。原Go默认差异、完整Go兼容与WP00-R/全部后续Phase不减免；没有现场TC/DB运行，共享锁继续延期，外部TC风险仍未接受。

## WP-01批准首期协议组件门禁通过（2026-11-13）

[正式peer-profile运行174850](phase-1/evidence/protocol-peer-profile/README.md)将固定Java实际字段/独立侧车与正式Rust、显式Go0001–0006的真实API做完整比较：628正向（576载荷/52帧）全部字段树及重新编码字节匹配；2050畸形输入不被完整输入入口接受。注册wire/default版本、report255、nullable头、i64、请求/子ID与失败结果均分别核对，未用任一待测decoder生成预期。

74正式组件测试＋1验证器回归在Rust1.99/最低1.85各通过，fmt/Clippy/doc严格门禁；7Python与12实际输出破坏负控通过。复审发现验证器把重编码错误当正常拒绝的问题，经真实运行RED1后改为独立encode_error，不能被计作负向通过。原Go714、候选714/24/13、1936编译依赖、1847输入及20产物闭合；1914成员catalog SHA256=8a5ba6f717bb2f9be1a01b7830620304b3378e3c8aa1ae6918a0f42c3c577fdb。复审独立重放与全部SHA通过。

[门禁记录](phase-1/wp01-protocol-gate.json)现验证通过，按已经批准的Java权威语义＋显式Go适配首期profile解锁WP02。没有新Java运行/现场TC，也未将原Go默认缺口改称兼容；Go254/旧probe117不计本轮重跑。完整Go兼容、WP00-R、后续全部阶段与发布仍未完成，外部TC风险未接受，共享锁继续延期。下一步实现正式连接层、分片/连续帧与请求关联。

## WP-02正式流与编号基础（2026-11-13）

已加入seata-transport，完成单帧增量解码、分片/连续输入、错误/EOF终止、元数据Debug/容量监控及单连接不重用signed-i32编号。协议层新增共享固定头前缀检查，完整帧API仍严格拒绝尾随；按现有NET规范先补实现决策和失败测试，没有新初始化规格体系。

[基础正式运行182417](phase-1/evidence/transport-foundation/README.md)11项新transport测试在1.99/最低1.85各通过，46Java认证帧分段/单字节、1000连续帧/恶意长度容量和ID额度边界验证；workspace静态/文档门禁、旧probe117新回归通过。718输入/741成员/5binary，catalog=ab4ff9fd23accc104dc0ec2962439a8e58ffbcc6fe620f1b734c41bef1902f18。新peer181728的74协议＋1验证器/MSRV、628/2050新回归通过，catalog0894066182ecae3458b9804a43318f01031b3cb4ffc7d0422c1707e2c0ff32a4，69编译输入绑定；旧README补充说明不冒称当前所有1848输入零漂移。

只勾选WP02流解码与ID分配2/12，完整NET007/WP02 Gate未完成。下一步实现实际异步TCP会话、pending完成所有权及写入后未知结果，再推进超时/重连/资源注册。原Go默认差异、WP00-R/完整V1/后续全部阶段仍未完成，共享锁延期、外部TC风险未接受。

## WP-02真实TCP单会话与等待请求（2026-11-13）

已实现拥有读写任务的TcpConnection、可克隆但不迁移代际的ConnectionHandle、有界队列/等待数、私有pending表和checked generation。注册先于发送；每项write marker决定本地失败或UnknownOutcome；响应移除/oneshot移交与期限/取消/关闭共用完成仲裁，真实迟到/重复只观测。显式shutdown与owner Drop均清理等待项和socket，3/4心跳不误完成普通请求。

[正式191253](phase-1/evidence/transport-session/README.md)33项在Rust1.99/最低1.85分别通过（9unit、10stream/ID、14真实TCP测试函数）；fmt/Clippy/doc严格门禁通过。审查发现调用timer未poll时仍接受deadline后响应，真实断言RED1后在接收仲裁处修复并复审通过。两个活动socket同wire ID、signed-wrap并发逆序/额度、Failed字段保留、批次末类型/数量、本地无效请求不写入、取消/关闭均有断言。2546输入/2570成员/6实际binary，catalog=bde63f35709b2ef4cf9472d3a65a998424cc13623039815d7683ff9a92833fe3；16锁依赖archive checksum和实际解包源码封存。

根workspace锁定Tokio1.54.1，所有依赖从已有缓存离线使用。新peer191025的74协议＋1验证器在两版本、628/2050与7Python/12破坏负控通过，catalog=e89f562278c942a3de7af0575357a93e66045dd563b992d25ef62109cf2b5c87；实际协议/Cargo编译输入绑定，未新执行Java/旧probe117或启动TC/MySQL。首个全平台metadata缺本地WASI archive的诊断保留，补现有缓存后完整门禁通过，不计产品RED。

WP02目前7/12；NET001–007传输组件场景已覆盖，完整WP02 Gate仍OPEN。下一步是连接池、定时心跳/idle、重连退避和认证/RM重新注册；NET008–012业务协调、TM/RM/MySQL和全部后续计划继续执行。raw connected不等同业务ready，运输层不自动重放事务；Go默认兼容/WP00-R、外部TC风险未接受与共享锁延期边界保持不变。

## WP-02定时心跳与完整帧读空闲（2026-11-13）

已完成显式ConnectionIdleConfig、同一生产writer上的空闲PING3调度、同一reader上的完整有效帧读idle检测。业务写/PONG写刷新writer计时，跳过取消残项不刷新；有效完整帧刷新reader计时，持续半帧滴流不能保活。PING共享每连接不重复ID但不建pending，额度耗尽关闭而不重发业务；ReadIdleTimeout按各write marker清理等待项，关闭/Drop复用原监督。

[正式192909](phase-1/evidence/transport-heartbeat/README.md)47测试在Rust1.99/最低1.85分别通过（14unit、9heartbeat target含8真实TCP/1config、10stream/ID、14既有真实TCP），workspace fmt/Clippy/doc严格门禁通过。三项复审覆盖缺口补齐后，四项隔离行为变异控制均实际断言失败；负控复用一个独立D副本逐项从基线重建，三种源变异、四个目标测试，不称原始产品RED或四个独立目录。2556来源/2585成员/8binary，catalog=bd05a0602c9e4172e50affc6bd159c186227ea1407b4b19072367598e57caaf9。只读终审核对全部SHA、依赖源码/五Java源、编译绑定和负控，未见本轮自有进程残留。

新增dev-only Tokio test-util功能；16运行期锁包未增加。新peer192819的74协议＋1验证器在双版本、628/2050/7Python/12实际输出负控通过，catalog=b8ddd7aac9ee5c776a9268f74bb1ffeec09c7939907aaf905b50438bcf84e9e2。没有新Java/TC/MySQL或旧probe117/Go254执行，没有安装或联网。

WP02当前8/12；完整Gate仍OPEN，下一步连接池、重连退避、认证/缓存资源重新注册，再推进NET008–012与TM/RM/MySQL及全部后续计划。心跳不表示业务ready，不改变Java TC事务重试权威。Go默认兼容/WP00-R、共享锁延期和外部TC风险未接受边界不变。

## WP-02有界raw池与重连生命周期（2026-11-13）

已实现ConnectionPool固定物理槽位、可克隆PoolHandle轮转获取、原代际PoolInbound有界转发、明确Connecting/Connected/Backoff/Stopped快照、正指数饱和退避与稳定窗口重置、旧会话join后替换和关闭/Drop任务监督。Pool请求只派发一次，不重试业务。Connected明确只是raw TCP，不冒充已认证/RMready。

[正式195040](phase-1/evidence/transport-pool/README.md)62在Rust1.99/最低1.85分别通过：19unit＋10pool target＋9heartbeat target＋10stream/ID＋14原TCP，总31真实TCP函数/2配置测试。100并发acquire只创建/复用两物理通道；掉Begin响应后新通道只接收显式Status、旧handle不迁移；实际退避60/120/200ms、稳定后60ms；入站满关闭并保留原代际；关闭中止connect/唤醒acquire及Drop活动等待均验证。

两P2真实RED后修复：quota用尽同步关闭旧代际并返回原本地错误，避免池永久选择耗尽通道；唯一绝对deadline在锁检查前建立并贯穿pool/session，避免60ms锁等待后仍剩4.999514s。五隔离变异实际断言失败（同一个D副本逐项baseline重建），不是五个原产品RED。workspace fmt/Clippy/doc严格通过；2564来源/2599成员/10binary，catalog=b326692a210b89b945a73b3cd53406de0da0b2ac18fd02195c1aaa9389774586。只读终审无新阻断且未见本轮残留进程，16依赖checksum/1850源码文件核对。

协议编译输入未变，绑定既有192819的628/2050/74＋1双版本/7Python/12负控归档（70编译输入相同），本轮不冒称新跑peer/Java/TC或旧probe117/Go254。没有安装、联网、分支操作或新依赖。WP02当前10/12、Gate仍OPEN；下一步每新会话身份/缓存资源注册ready与NET008–012，再继续TM/RM/MySQL和全部后续阶段。原Go默认兼容/WP00-R、共享锁延期与外部TC风险未接受不变。

## WP-02每新会话身份/资源登记与修订就绪（2026-11-13）

已实现SessionRegistration单角色TM/RM策略、显式有界RegistrationIdentity、typed ResourceKey/ResourceRegistry修订缓存、完整逐项报告和Registering/Ready/Degraded状态。每个新代际全量登记；动态insert使旧ready立刻失效并重新登记同一活会话，初始化中改缓存取消旧future且迟到回复不会放行。拒绝/Local/Unknown/NotAttempted分别保留，整轮使用同一绝对期限；注册中shutdown/Drop关闭该代际，单通道close不重发事务。

发现资源型auth策略不接受额外RM(None)身份步骤，真实TCP断言RED后改为首必需资源同时建立身份，报告两视图共用一次attempt；空缓存才RM(None)。[正式202518](phase-1/evidence/session-registration/README.md)76测试在Rust1.99/最低1.85各通过（20unit/42真实TCP函数/10stream/4config），六隔离变异均实际断言失败；workspace fmt/Clippy/doc严格通过。2590来源/2628成员/12binary，catalog=38f8255d69b57f15eaeb43ea05a3b7927c079d76014296e8eff9eae75437f107。

[新鲜Java现场202129](phase-1/evidence/registration-java/README.md)使用原JAR和已有公共认证SPI，六用例证明TM/RM允许/拒绝、两个资源重连全登记、动态第三资源拒绝后not-ready；五真实类加载来源、runtime扩展两文件、原JAR/fixture哈希及122同编译输入核对，FILE store0、自有TC已停/38091释放。133来源/146成员/1正式client binary，catalog=3c6bcb2027de70623779f9eaa942dae9b831fba1732fed5aa817f5780622224f。旧201818同六功能用例成功但缺增强来源记录，保留历史，不虚增用例数。没有Java重编译/JUnit，父门禁只引用先执行的Java子归档，不能因此声称整个里程碑没有新Java执行。

只读终审逐来源/产物/依赖/负控/类来源核对通过，无本轮残留进程。协议70编译输入仍绑定已有peer192819，未重新执行628/2050；16Rust锁包不增加、无安装/联网或分支操作。WP02当前11/12、Gate仍OPEN，下一步NET010–012事务响应丢失/协调，再推进TM/RM业务、MySQL及全部后续阶段。原生identified并非生产认证或RM补偿证明，原Go默认差异/WP00-R、共享锁延期、外部TC风险未接受不变。


## 2026-11-13 — WP02正式原代际回包与NET001–012组件Gate通过

[组合212118](phase-1/evidence/transport-recovery/README.md)87正式transport测试＋另3联调辅助测试在Rust1.99/最低1.85各通过；24unit、49真实TCP函数、10stream/ID、4config。workspace fmt/Clippy/rustdoc严格通过，14隔离行为变异均实际目标assert失败exit101；13Python含13原始帧/状态破坏被拒绝。2634来源/2695成员/16实际执行binary，catalog=ba1458a8a1e944e4693a552fe6ef201db5b4b2457646170e5aca70637006c5f2；16依赖archive及1850包源码核对。

PoolInbound消费原代际能力，支持原signed ID/nullable有序头/精确XID与BranchId的sync回包；独立有界回包额度与普通future表隔离，同ID不交叉完成。完整本地写完才成功，写前取消跳过整帧，已开始写必须完成或关闭socket。实际产品RED发现关闭先发生仍发布成功、无Tokio上下文却已入队；修复后双版本GREEN。观察器u16布尔值与raw关联/运行配置/成功发布次序缺口亦有RED/GREEN，早期诊断不计最终认证。

[登记Java211350](phase-1/evidence/registration-java/run_20261113_211350_6a0bee84/result.json)六现场用例重跑通过：139来源/152成员，catalog=8ac6fd9f4a2c2e1af44be92ea9f3ed9a1bc35479e73b0fd89815cacb8fe2487f。新[丢响应Java211914](phase-1/evidence/transport-loss-java/README.md)三个独立TC＋三个代理通过：143来源/185成员，catalog=76219d90eeb6cdde8a7ab7d67fddf083815545d91af6bfb83f71a5ef8abe8519。NET010显式查询/同XID同操作重复Commit，不转Rollback，实际Finished15/15保留confirmed=false；NET011只发一次Begin、客户端无观察器XID；NET012不重注册，仅从真实匹配rollback callback学原BranchId、TC明确Rollbacked11，模拟动作一次/锁释放。

只读终审核对父/子catalog、各128共同编译输入、26实际Java类来源、三份实际运行配置、56原始帧和状态顺序/元数据。伪造客户端PASS而保持原wire15/15被父审计拒绝。所有自有TC/代理及38091/38092清理。父脚本自身没启动Java，引用本里程碑新Java子运行；未编译Java/JUnit/MySQL或重跑旧probe117/Go254。协议peer192819的70编译输入一致，628/2050等仅引用未新跑。发布README/清单/approval是执行后更新，不宣称所有旧文档来源当前零漂移；当前编译输入不变。

[WP02机器状态](phase-1/wp02-transport-gate.json)已验证，清单12/12，下一步WP03正式TM API/异步上下文/期限及未知结果协调。范围仅已批准首期transport组件及所选NET故障场景；生产TM/RM、持久幂等、SQL/Undo补偿、生产认证/TLS、原Go默认兼容/WP00-R、全部后续阶段与发布仍开放。共享锁继续延期，外部TC风险未接受。无新依赖、安装、联网或分支操作。


## 2026-11-13 — WP03正式TM组件、来源约束恢复与基础Java验证

[221732组合证据](phase-1/evidence/tm-component/README.md)28TM测试（7配置/上下文、13调用/恢复边界、8作用域）及87传输回归在Rust1.99/最低1.85各通过；10隔离行为变异都触发实际目标断言失败exit101。workspace fmt/全target Clippy/rustdoc严格通过。2058输入/2122成员/22实际执行binary，catalog=a41801b7fcad2ba66f28dd99046c5f62a5f45ad7248621e9c4fa4a5dfaeaeed8；16依赖archive及1850解包源核对，无新依赖/安装/联网或分支操作。

正式seata-tm包含登记TM池builder、独立网络/TC期限、精确XID/可选字段、词法task context、六传播策略、Begin/Commit/Rollback/Status和完整Typed错误。业务错误与回滚错误同时保留；UnknownCommit不转Rollback、不自动重发；Finished不能确认历史提交，失败查询撤销旧许可并保存TC retry-timeout状态。panic/取消恢复本地上下文，不启动无所有者的网络清理。作用域错误可用本管理器真实Launcher签发的私有RecoveryToken恢复原XID/原意图，改错误字段、伪造诊断、跨管理器均拒绝；先查询且不申请新TC期限。

接口首个RED为缺API编译错误。后续真实断言RED修复附加数据/名称Debug泄露、Failed/Finished擦除原未知、失败查询沿用许可，以及公开诊断可切换Commit→Rollback/伪造恢复能力。两份来源RED后加入私有manager/XID/op证明，负控验证忽略issuer仍会被测试抓住。早期220916虽然27测试/9负控通过，但恢复来源P2未关闭，仅作历史诊断，不能替代本最终组件记录。

[Java221553](phase-1/evidence/tm-java/README.md)使用原固定TC/JAR及公共认证SPI，六功能场景/七个空全局事务实际通过；167来源/179成员/catalog=dcbb3a1ed9c0671dfc9f50a323b84c15356114dc9e72e1f8df0150d8b8831e5d。原始stdout字节7Begin/4Commit/3Rollback，七XID/nonce/50ms与60000ms期限、6实际类来源/实际配置/SPI、client binary和自有PID54740/38091清理均核对。Java提交确认后的异步清理用至多5s只读状态查询观察，七个最终Finished；没有重发事务或以Finished倒推历史提交。初215516仅收尾立即Finished假设失败，不计成功；其后历史成功因源码更新均被最新221553取代。

新[peer215647](phase-1/evidence/protocol-peer-profile/run_20261113_215647_eee62e75/result.json)在新TM工作区实际执行74协议＋1验证器双版本、628正向/2050畸形、7Python/12实际输出破坏控制；1849来源/1916成员/catalog=23b5d8072aee9037757604763d5c073164dc564394f97f9702e3418a2c817919。71协议/Cargo输入与父一致，显式Go0001–0006实际API运行，原Go默认兼容未关闭。未执行新Java编译/JUnit、MySQL、旧probe117/Go254或再次完整旧transport负控；Java子运行是本里程碑新执行，父脚本自身不另启动TC。

只读终审核对全父/子catalog/源快照、22产物、155共同Rust/Cargo输入、16archive/1850依赖文件、10负控patch/恢复副本及Java日志/来源/配置/清理通过。随后发布README/清单/approval/状态不宣称旧文档当前零漂移，编译输入单独保持一致。[WP03状态](phase-1/wp03-tm-gate.json)记实施组件12/12，**完整Gate仍OPEN**；下一步正式TM对原JavaTC的真实响应丢失/不确定性恢复，不能以旧transport驱动代替。RM/SQL/Undo、跨进程持久恢复、生产认证/TLS、原Go默认/WP00-R、全部后续Phase与完整V1发布仍开放，外部TC风险未接受，共享锁继续延期。


## 2026-11-13 — WP03正式TM真实响应丢失与完整组件Gate通过

[224638组合门禁](phase-1/evidence/tm-gate/README.md)绑定28正式TM＋87传输回归和另2联调辅助unit，Rust1.99/最低1.85各通过；19隔离变异（10库＋9观察器）均真实目标断言失败，13观察器＋7proxy Python通过，workspace fmt/Clippy/rustdoc严格通过。2090输入/2175成员/24实际测试binary，catalog=f59fdd96a66d2a1b8768a9c0317bbe1464fe97e7797847ba66798bb11f92e4d2；16archive/1850依赖源逐SHA核对。

新[Java224303](phase-1/evidence/tm-loss-java/README.md)三个独立原TC＋三个代理，正式execute/recover_unknown路径的Begin/Commit/Rollback成功响应丢失通过。175输入/216成员/catalog=76f1949973528f422db0a5b9fa583d172f47eabcff935736ba7031ecd2824f63。Begin无XID、不构造业务、不再次发起；149字节FILE证明原执行，观察器身份未提供客户端。两个结束模式使用真实原分支回调，内存动作各一次/锁释放，同管理器原凭证只恢复同意图、查询后显式重复一次；实际15/15仍unknown_retained=true，Rollback原业务错误保留。58原始帧，唯一实际响应drop、原15请求在drop后且16在retry前；没有改写帧或无依据声称历史成功。

首轮223750仅功能成功，验收器误允许重复投递原响应和旧在途查询冒充恢复；新两方法出现实际四次断言RED，修后13方法GREEN，新epoch重新跑224303。旧support及正式TM库未改。九D独立Python验收器削弱变异全部被反例断言抓住，不能把API初缺模块红误称运行失败。

引用此前Java221553六基础场景/七空事务及6类来源，155共同编译输入一致，未在本轮重跑；新fault子156共同输入，仅新增driver，原库不变。基础6＋故障3×7=27实际类来源、原配置/实际runtime配置/SPI/JAR/client产物和清理均核对；父自身不启动TC，子本轮确新启动3TC＋3proxy。未新编译Java/JUnit/MySQL、旧probe117/Go254；协议peer215647的71相关输入及既有628/2050、74＋1双版本/7Python/12输出负控只引用，未冒称本轮重跑。

只读终审父/子全catalog、155/156/71编译输入、24产物、16archive/1850源码、19patch与候选恢复、58帧/来源/配置/SPI/清理通过，无本轮进程及38091/38092残留。发布README/清单/approval后另查编译输入，不宣称所有历史进度文档零漂移。[WP03状态](phase-1/wp03-tm-gate.json)已验证，12/12实施与完整所选TM Gate关闭，下一步WP04正式RM控制面。

该Gate未完成生产RM分发/持久幂等、SQL/Undo、生产认证/TLS、跨进程恢复、原Go默认兼容/WP00-R、全部后续Phase、完整V1或发布。共享锁继续延期，外部TC风险未接受。没有安装、网络获取、新依赖或分支操作。

## 2026-11-14 — WP04正式RM持久恢复基础已验证，完整Gate继续

[最终基础记录](phase-1/evidence/rm-foundation/README.md)新增正式seata-rm资源目录、显式恢复适配器、共享租约分派及有界追加日志。先同步Pending、未决先查询、完成先同步；取消的后台磁盘工作保留分支/预算许可，同租约变更也串行。真实取消断言RED修复旧写晚到窗口；初API RED为缺模块/接口编译失败，容量夹具200→160纠正不计生产缺陷。

Rust1.99/最低1.85各212正式测试（74协议＋87传输＋28TM＋23RM）通过，46实际测试产物；RM为1真实write失败/poison、12日志、8分派及2强杀恢复。每版本提交/回滚各3子进程，效果已同步而RM完成未写时父kill/wait，两个新进程恢复/去重，apply/recover各一次，共12子进程。默认ignored child入口确由父启动，不额外充正式测试。CRC/身份/先Pending/先恢复/完成缓存5D盘隔离削弱均编译并在目标断言失败；RM格式/Clippy/rustdoc严格通过。

最终run_20261113_235728_c0b4952f：2604输入/2723成员，catalog=5b14b429114e2ebcdd6295bf6a48febcecc56ee40b4d48699af75d468d74d17c。22锁定archive及18当前平台依赖1835解包源码逐项比较、前后冻结；其他4只验证archive，不声称其他平台测试。早期235227未冻结依赖源码结束状态，经审查修复后重跑，最终记录不能被旧功能成功替代。只读终审全catalog/46产物/12子进程/5目标负控通过。

新[peer235407](phase-1/evidence/protocol-peer-profile/run_20261113_235407_74265da4/result.json)在四crate工作区新执行74＋1双版本、628/2050、7Python/12输出破坏控制，1850输入/1917成员/catalog=11e3fd8a6afca7c2e507a3ecc39df07b67dc9b6efb87895ba04f439320ea5bfa；显式Go0001–0006，原默认仍开放。发布后独立审计foundation2035编译/依赖输入、peer815代码输入当前匹配，共享72项一致；历史清单/approval发布变化单独列出，不声称全部历史文档当前零漂移。没有新Java/JUnit/现场TC/数据库、CLI安装或分支操作，全部开发/运行输出在D/E盘。

[部分状态](phase-1/wp04-rm-foundation.json)不关闭任何完整WP04验收项。下一步拥有网络生命周期的注册RM、BranchRegister/Report/LockQuery、完整原连接回调/平坦批次与Java TC重复/重连Gate；真实资源NotApplied迟到围栏、独立sync故障和同租约操作序列独立负控仍待验证。文件效果夹具不证明SQL/Undo原子性。生产认证、WP00-R、原Go默认、完整V1/发布及其后Phase继续，外部TC风险未接受，共享锁暂缓。

## 2026-11-14 — WP04正式RM运行组件已验证，真实Java Gate下一步

[运行组件证据](phase-1/evidence/rm-runtime/README.md)实现ResourceManager拥有原生登记池/有限回调JoinSet，ResourceClient单次BranchRegister/Report/LockQuery，完整原请求/TC Failed/未知及协议分类；报告可选资源保持None，不补BranchId/自动重发。原连接signed ID/可空heads、完整flatbatch预校验、原绝对期限、磁盘排空及关闭失败报告已接入正式资源分派。已有初始化期间有界缓存回调、完整revision Ready后转发/发业务的合同保持不变。

Rust1.99/最低1.85各234正式测试（74协议＋87传输＋28TM＋45RM），52实际产物。RM为3unit/12journal/8dispatch/2进程恢复/5calls/8callbacks/7edge；12真实恢复子进程及apply/recover各一次再次执行。11D盘隔离削弱全部编译并在指定目标行为断言失败，含丢清理panic、Failed覆盖、非法成功ID、发送未知标记、批次方向、校验期限等。格式/全target Clippy/rustdoc严格通过。

源码审查两P2已修：关闭收尾吞掉资源Future析构panic→真实目标RED后保留非取消JoinError并清理后Err；整批期限误从parse后重启及超限app先复制→入口deadline/借用校验/seen索引，2边界测试及期限负控。初native API缺类型编译红、时钟目录碰撞夹具修正分别记录，不混称生产行为缺陷。动态资源初试验错误要求TC登记ACK前等待回调，冲突于既有合同；试验泵循环已逐字节撤回，不变更核心标准。最终PONG仅证明reader进度，最后ACK后原回调/单次业务各出现，不能夸称dispatcher调度屏障。

最终run_20261114_003214_ee6afbb3：2615输入/2756成员，catalog=66e1711307ef6a78c94ed1bde0acdd9e1ce1492bb035d6fbf133d9599490cad4。22archives、18当前平台1835实际依赖源前后冻结，其他4仅archive身份。只读终审全catalog/source/52产物/11目标失败/四组原身份CRC状态效果/12子进程均通过，无残留测试进程。发布后独立审计2046编译/验证器/依赖输入及52实际产物当前相同，与继承peer235407的72相关编译输入一致；只有RM README和清单发布MD变化，不宣称全部历史文档零漂移。新Go peer/Java/JUnit/真实TC/数据库并未在本轮执行，无安装、分支或C/F写入。

[当前部分状态](phase-1/wp04-rm-component.json)只完成模拟TC下的运行组件，完整WP04仍OPEN。下一步固定原JavaTC正式RM登记、真实分支注册/报告/锁查询与提交/回滚、重复投递及重连Gate；不能以模拟TC45测试替代。真实资源NotApplied迟到围栏、独立sync故障、同租约操作序列独立削弱、MySQL SQL/Undo与后续全部门槛继续。外部TC风险未接受、共享锁暂缓、完整V1/发布和后续Phase均未完成。

## 2026-11-14 — WP04获批首期验收通过，继续MySQL AT

[合并验收](phase-1/evidence/rm-gate/README.md)完成原WP04十项与真实Java Gate。正式ResourceManager/ResourceClient与持久恢复实现不变，继承Rust1.99/1.85各234正式测试（45RM）、52实际产物、11实现削弱断言、12恢复子进程；当前2044编译/依赖输入核对相同，不声称本次重跑234项。新增原JavaTC正常/重复/断连各提交回滚六例，6TC/12代理/54实际类来源、96RM与580TM原始帧；资源均apply/effect1，日志仅Pending→Completed并重开，原申请/报告/事务结束均无自动重发，TC负责重试。

双端观察及原TC日志独立绑定nonce、两个原始XID、TC正i64BranchId、原资源/应用数据、报告/锁/结束/回调/交付回包/状态查询顺序。38Python、2辅助资源测试双Rust及10隔离观察器削弱目标断言均执行通过。断连回滚在5秒预算下客户端报告未知；原目标迟到Failed0/exception0/status5保留为独立观察，竞争者回滚必须明确11，所有状态逐项匹配raw16，Finished仅为清理观察；未独立证明精确超时时刻。

Java子014916：2075输入/2169成员/catalog c4763b18ce8143b932683d4a4c95a91fb57b6e7dfd3fabd0cd26147d3d574da2。最终父020104：2076输入/2113成员/catalog3583c327f3fd939a4a5ad007755ac9fa8d317c1cfcd844fe72f8d0612e733ef4。子全部2756+2169成员与快照重新核验；22锁定archives/18当前平台1835依赖源绑定。只读审查发现父控制执行文件CRLF与归档LF的exact-byte差异，已改为归档实际D执行文件原bytes并重跑父；最终10项执行/归档/记录SHA完全一致，旧015811不能作为最终封存。更早单端较弱观察、低分辨率时钟拒绝、合法迟到Failed误拒绝及catalog格式失败均保持历史，不以它们冒充最终通过或产品行为RED。

[当前机器状态](phase-1/wp04-rm-gate.json)将WP04标记为获批首期控制面通过；SQL/Undo、资源适配器迟到围栏、独立sync故障、生产认证、WP00-R、原Go默认及所有后续Phase继续开放。原初始化回调缓存/完整Ready规则不变，外部TC风险未接受，共享锁暂缓。自有Java/代理与三端口均已清理；没有新Java编译、JUnit、Go/peer比较、依赖下载、CLI安装或Git分支/提交操作。全部开发与运行输出D/E盘。下一步WP05真实MySQL AT phase1，然后WP06及其余原计划。

## 2026-11-14 — WP05 SQL识别首批验证，真实MySQL/Undo继续

[首批证据](phase-2/evidence/at-sql/README.md)新增正式seata_at有界SQL工厂、三类MySQL识别器与类型化结果/错误；单表INSERT VALUES/UPDATE/DELETE明确矩阵，保存原SQL/表/有序列/行数/绑定数/WHERE偏移，Debug及错误脱敏。真实行为RED覆盖IN列越矩阵、UPDATE FROM与LOCAL/LIMIT ALL被替代解析器消费、未支持运算链先构深树、裸内建项误当列；固定MySQL DIV的坏输入确触发上游unwrap RED，现先限定DML动词并拒绝未支持DIV，不改依赖源码。AST重建逐token等价补足字段为None的检查；注释/DDL/别名/跨schema/join/子查询/函数/变量及未支持形状明确拒绝。

最终023603_0d037bc7：2781输入/2927成员/catalog02382691b9dc5916063eba7147e5622769d9a70c2256c1a7c66fcc6eb39b3902。每Rust1.99/1.85新执行252项（74协议+87传输+28TM+45RM+18SQL）、56真实产物；10个隔离削弱实现成功编译并在指定断言失败，执行/归档/记录SHA相同；4组RM Commit/Rollback恢复与12实际子进程归档。格式/Clippy/rustdoc严格通过。初023055最低Rust回归发现旧TM测试固定20ms睡眠会割断测试端分片ACK写；仅换成有界oneshot后再完整重跑，信号只证明测试端ACK已写完，不声称客户端Ready。生产TM/RM/协议/传输149源未变，31原测试源未变；根工作区只加AT，原锁26包完整保留。

sqlparser0.52.0/log0.4.27由既有C公开缓存读取并checksum核对后复制D，未修改C缓存、无下载或CLI安装；24归档/20当前平台1894实际依赖源逐tar成员核验并冻结。只读终审全部成员/来源/产物/负控及恢复记录通过。Java8对象为PARTIAL，固定sqlparser测试目录85文件仅盘点，源测试案例未展开/未执行差分，所列目录非Java资产0，不声称全项目资产为0或Java完整迁移。

[WP05部分状态](phase-2/wp05-at-sql-component.json)维持0/14与Gate开放；本轮无真实MySQL/SQLx/Java/Go现场执行，没有镜像、Undo原子提交或提交未知围栏证据。继续真实MySQL元数据/主键与镜像、确定性Undo和同一本地事务，再完成WP06及全部后续原计划。生产认证、WP00-R、原Go默认、完整V1及原生RustTC仍未完成；外部TC风险未接受，共享锁暂缓。所有开发和运行写入D/E盘。


## 2026-11-14 — WP05真实MySQL元数据基础通过，镜像与Undo继续

[元数据基础证据](phase-2/evidence/mysql-foundation/README.md)新增正式seata_datasource/seata_db_mysql，SQLx0.8.6连接、本地事务、MDL、物理列/复合PK顺序、严格会话验证和单操作绝对期限。失败/取消时poison并丢弃连接；当前不暴露业务提交入口。实际部分撤权成功误判和响应扣留超时RED已修复，初权限夹具/解码失败独立保留。

最终035455_0dec6b55冻结7979输入/8126成员，catalog151350d6ba34f40d03a30521b313a77345ae4fe2f890fb09272854a8423188fc。Rust1.99/1.85各259工作区测试、66工作区产物、4隔离实现削弱目标断言失败、12既有RM文件恢复子进程；另各8项真实MySQL测试和两个独立live产物。现场8.4.13/lower_case_table_names2，覆盖MDL/Drop、不安全表、TRIGGER/PROCESS/部分撤权与43字节响应扣留；两个自有容器已清理。171归档及151活跃依赖6806源码逐项匹配。六Java对象PARTIAL、189源测试仅库存、七资产原字节复制，未新执行Java/Go差分。

[部分机器状态](phase-2/wp05-mysql-foundation.json)保持原WP05为0/14，WP06未完成。行数据精确解码、镜像、业务SQL、Undo及提交未知仍待实现；当前直接TRIGGER/全局PROCESS权限及loopback非TLS仅为已验证基础配置，不是生产最小权限/TLS验收。WP00-R、原Go默认、全部后续Phase、完整V1及原生RustTC继续；外部TC风险未接受，共享锁暂缓。开发和运行输出只在D/E盘。


## 2026-11-14 — WP05精确行值模型通过，数据库镜像继续

[行值证据](phase-2/evidence/row-image/README.md)新增Field/Row/TableRecords及精确DecimalValue/FieldValue，完整schema/独立PK序、NULL与空值、65位DECIMAL/30位scale、i64/u64极值和有界原子累积。最终run_20261114_042231_afaee4a5冻结7988输入/8148成员/catalog 8e2ebc241143742e847a3821e7b0e06bf5435d1f8f83112d26ab94340956156c；双Rust各273工作区测试、68产物、10隔离削弱目标断言失败、12既有RM文件恢复子进程。初042039校验器不识别自定义assert提示后修正并完整重跑，不混作产品缺陷。171archives/151活跃依赖6806源码再次核验，锁文件未变。

九Java对象PARTIAL、189测试源仅库存、七资产原字节复制；无新Java/Go/MySQL现场。前轮真实元数据证据保持历史范围。当前五类值域不能证明全部MySQL物理位宽/字符数/UNSIGNED DECIMAL合法性，SQLx解码与补偿前仍须补齐。WP05保持0/14，数据库行读取、before/after、业务/Undo原子提交及所有后续门槛继续；完整V1/发布未完成，外部TC风险未接受，共享锁暂缓。全部输出D/E盘。


## 2026-11-14 — WP05物理域及UPDATE/DELETE真实前镜像验证

[组件证据](phase-2/evidence/mysql-row/README.md)落实真实物理约束、原参数/原生DECIMAL、同事务长度预算预检与完整持锁前镜像。最终run_20261114_045819_c62f4e63为7952输入/10224成员/catalog 10285b1534c237d5f1eab8944aa039851a70a9798eedd0f80a66bccdb1993ea6；双Rust各282普通+19真实数据库场景，72工作区与4现场正向产物、8普通+5现场目标assert负控、12既有RM恢复子进程。185archives/真实148编译registry的6745源逐项核验；metadata159候选不混称活跃，旧151/实际146另有统计范围澄清。所有原178锁包版本/checksum保留；协议/传输/TM/RM/AT源码不变，旧Java/Go Gate未重跑。

取消现场先证明MDL阻塞、取消后在代理关闭前证明释放；原始43字节只证明响应扣留。general_log核对预算拒绝未读取完整值。九Java对象PARTIAL+两方法引用、189原测试仅库存、七资产原字节。九自有容器均清理，数据目录D盘保留；8.4.13/lowercase2/loopback非TLS，生产TLS/最小权限未验收。WP05仍0/14；两查询间插入/字段膨胀精确竞争、INSERT镜像、业务/after/Undo原子性及后续全部计划继续。完整V1/发布未完成，外部TC风险未接受，共享锁暂缓。


## 2026-11-15 — WP05两查询间并发变化现场通过

[竞争证明](phase-2/evidence/mysql-image-race-gate/README.md)在完整值查询COM_STMT_PREPARE转发前扣住102B原包，独立BLOB增长及范围INSERT均1205；放行后原一行/一字节，rollback后两操作均成功。Rust1.99/1.85各新执行一例，去预检锁/改RC两实际削弱分别在增长/幻行目标assert失败。最终父run_20261114_052627_abb1eddd为7955输入/8502成员/catalog cac7531795b6296a541b81c61841e9c2552846b87539079dc7f412225d79311a；生产182源与既有223.rs、Cargo全部原bytes，原282+19每版及13负控只绑定继承，不声称新执行。四自有容器清理。

只读终核完整归档通过；D临时目录之后已清理，旧运行源/exe缺失明确记录。已从E封存及C公开索引恢复185archive和6745源码，6930原缓存文件逐SHA一致，无网络/工具安装/Cargo锁变动，不重建旧exe。WP05保持0/14，INSERT/业务/后镜像/Undo原子提交、WP06及所有后续计划继续；完整V1/发布未完成，外部TC风险未接受，共享锁暂缓。


## 2026-11-15 — WP05受限业务执行与前后镜像

[最终组件](phase-2/evidence/mysql-execution-gate/README.md)冻结7971输入/10996成员/catalog 07bed67b72f1501d5dcc7877aa498b1dc0fd186f67696571b00fc14293a45d5e；双Rust各291普通+37真实MySQL场景、80工作区及8正向现场产物，5普通+6现场削弱目标失败，12既有RM文件恢复子进程。修复字面量主键CAST误捕旧行和128条预算状态漏洞；补齐赋值/权限/累计字节、取消/超时与命名冲突现场。14自有容器清理，185registry checksum未改，实际148编译依赖6745源码校验。

只有事务内DML/完整镜像/内存Undo，尚无COMMIT、Java日志互通、持久化/原BranchRegister/未知提交恢复。14 Java对象PARTIAL、189源测试仅库存。WP05仍0/14，WP06及所有后续计划继续，完整V1/发布未完成；外部TC风险未接受，共享锁暂缓。


## 2026-11-15 — 原生类型身份与原Java Undo格式对照

[组件证据](phase-2/evidence/mysql-type-gate/README.md)为7992输入/9275成员/catalog fe2928425b3ec7bd3e725f5b104f948548756f0f6f648869e571103fb13fdc6b；双Rust各297普通+38真实场景、84工作区及10现场正向产物、2普通+2现场削弱目标断言失败、12既有RM恢复子进程。保留18原生类型和tiny显示宽度，旧构造None；真实缺失身份RED已修复。

绑定Java18选定原源/32class/335runtime实际来源；8.0.27两配置各6表、16原生成资产，默认{}及Empty具体类均实际比较。5.1.42两配置query_cache_size连接失败；其他helper版本为明确混合冻结profile，不称完整原Maven或Java案例迁移。Rust-Java codec、Undo原子落库/注册/未知提交及全部后续仍开放。WP05仍0/14，外部TC风险未接受，共享锁暂缓。


## 2026-11-16：选定Undo文档/实际准备日志编解码

[最终组件记录](phase-2/evidence/undo-codec-gate/README.md)与[机器状态](phase-2/wp05-undo-codec.json)完成本步：可空原对象图、精确Java类型/数值、有界JSON、真实四准备日志双JDBC配置。最终双Rust各316普通回归、各1个实际codec现场（双profile×4log）、40/40双向Java文档及12个新RM恢复子进程通过；旧每版38个其他DB场景和5+1控制明确继承修复前整轮，不能说最终源码新跑39。两处审查P2有真实RED→GREEN且终核无剩余P1/P2；节点预检原实现正确。185archive/150实际编译包/6871源码未升级，原7资源的checkout-vs-Git行尾来源已纠正并保留原字节。

1135个源测试方法声明仍MISSING，具体运行案例未全部展开，原JUnit未执行、原完整Maven环境未重建。无同事务Undo持久化/真实TC分支登记/提交未知恢复或公开COMMIT；WP05原14项仍0/14、WP06及后续全部计划继续，外部TC风险未接受，共享锁暂缓。下一步在原事务边界接入Undo原子落库与登记/围栏，不能由编码字节推断已完成原子性。


## 2026-11-16：原主键锁键与事务锁集合

[最终锁键组件](phase-2/evidence/lock-key-gate/README.md)与[机器状态](phase-2/wp05-global-lock-keys.json)新增原BaseTransactionalExecutor锁键及ConnectionContext锁集合部分；修复主键索引序与原Java HashMap次序混淆、禁止事务内混JDBC配置。两版Rust各325（316既有+9新）测试，1029原方法格式/次序MATCH，实际MySQL每版双配置×9日志锁键（含16列碰撞PK）及4原codec日志MATCH，40/40文档对照新执行通过；8普通/1现场负控命中目标断言，96产物/12RM子进程/3容器清理有证据，终核无P1/P2。178既有生产源与7056依赖输入未变，旧38 DB场景明确继承。

这不是TC锁覆盖/登记、完整Java对象或源JUnit、Undo持久化/原子提交证明；WP05原14项仍OPEN。新增[TC覆盖核查](phase-2/tc-lock-coverage-investigation.md)记录原来源方法录制后端的未完整资格化诊断：空白键无行仍返回true、分号片段提前结束、byte列表逗号拆分；没有真实TC网络/锁存储结果，不自动接受外部风险。下一步正式验证这些覆盖边界并接入真实BranchRegister、同连接Undo及提交未知围栏。共享锁暂缓、WP00-R/完整V1及全部后续Phase保持原范围继续。

## 2026-11-16：原TC锁覆盖的真实网络与存储观察

[选定观察与边界](phase-2/evidence/tc-lock-live/README.md)将15个原来源方法案例接到8组真实MySQL镜像生成键、未修改原TC数据库存储和两路独立原始帧观察。16个原XID、16次Begin/登记/回滚、8次锁查询、13次实际回调、25个真实DB确认点，最终global/branch/lock及业务/Undo均0；自有进程与精确容器清理。空/空白主键无锁登记、多行空值及分号漏锁、不同二进制主键额外冲突得到明确记录；普通同键冲突正控制正常。观察符合预期包含缺陷，不代表TC安全或生产登记整体门槛通过。

16项实际证据副本负控制均命中目标拒绝；新客户端Rust1.99 fmt/clippy/2项夹具测试与Rust1.85同源构建/2项夹具测试通过，原方法1029/文档40对照重新执行；280个crate输入保持前一锁键组件字节，原325普通回归和旧38其他DB场景未重跑。185候选归档/150实际编译包未升级。原TC现场限定retry-dead-threshold1000ms、外部JDBC8.0.33及非TLS本机测试，默认70秒时序不在完整现场通过范围；另一个bin文本collation读取失败保留未完成。最终只读审查无剩余P1/P2。

WP05原14项仍0/14，原JUnit具体案例展开、Undo同连接持久化、真实登记围栏/提交未知恢复、WP06与全部后续Phase继续。外部TC风险未接受、公共COMMIT未启用、共享锁仍暂缓；不通过改写原键或缩小核心验收消除缺口。

## 2026-11-16：同事务Undo写入与回滚

[最终组件记录](phase-2/evidence/undo-store-gate/README.md)与[机器状态](phase-2/wp05-undo-store.json)实现原七列Normal写入、实际packet80%分片、唯一身份仲裁及成功封存。原独占事务覆盖业务和Undo，错误/取消/期限到保持poison并回滚/丢弃；已有Normal/GlobalFinished不覆盖。严格验证原表、双向外键、触发器和权限可见性。新MySQL manager按原末级mysql包落位，源对象仍明确PARTIAL。

最终双Rust各331个普通/示例测试（原325+原示例6）、各50个真实DB现场（新10+Ready边界1+原38+codec/锁1）、各4个原Java存储方法对照及2个子行身份负控通过；112测试产物、12个新RM恢复子进程、16个自有MySQL清理均封存。三项真实RED→GREEN与10项证据角色负控有目标拒绝；185归档/150实际包未升级。最终全workspace fmt/clippy/all-features、doc及只读审查通过。原文档40、锁1029和TC来源15本轮重跑；上一轮实际TC网络8例明确继承。

本步是同事务写入与回滚组件，尚无原子COMMIT/真实登记绑定/提交未知围栏或生产二阶段补偿。压缩当前NONE，UUID_SHORT非原ID策略迁移；完整manager/JUnit、bin文本解码、外部TC覆盖问题仍OPEN。WP05原14项仍0/14，WP06和原全部后续Phase继续，未接受外部风险，共享锁仍暂缓。


## 2026-11-16：原唯一身份的持久防悬挂

[最终围栏组件](phase-2/evidence/undo-fence-gate/README.md)与[机器状态](phase-2/wp05-undo-fence.json)完成独立控制事务GlobalFinished、迟到Normal唯一键仲裁、重复观察、提交前错误/控制提交未知分离。真实并发发现RR旧快照可能漏读锁定查询已见guard，定向RED→GREEN后状态与内容均使用当前读取。Fenced仅是保留控制记录期间的防迟到事实，不直接变成通用RM Applied/NotApplied。

双Rust各331普通/示例、各24本次DB检查（9围栏+3原包故障+1旧快照+10同事务写入+1Ready）及各2原Java guard/4存储方法对照通过；116归档程序、12新RM恢复子进程、150实际依赖集合保持既有版本。30原始包与6角色副本负控命中目标；源码/依赖、fmt/clippy/all-features/doc、终核无剩余P1/P2。观察器停止失败的实际控制仍完成清理，资格15个容器及本轮全部37次尝试容器均独立确认不存在，D数据保留。其他39项DB明确继承，未计成本次执行。

业务COMMIT/Undo原子提交、正式TC登记/资源恢复绑定、真实二阶段补偿、原完整对象/JUnit、生产TLS/多节点、GC及原TC覆盖/bin文本缺口继续。WP05原14项仍0/14，WP06及原全部Phase未完成，外部TC风险未接受，共享锁仍暂缓。下一步把已确认的原身份围栏接到资源恢复与真实Undo处理，保留提交未知身份和TC协调边界。


## 2026-11-17：原Normal Undo真实补偿

[最终补偿组件](phase-2/evidence/undo-rollback-gate/README.md)与[机器状态](phase-2/wp05-undo-rollback.json)完成原Normal/子片读取、全部身份/物理元数据/JDBC恢复、完整PK元组当前镜像比较与INSERT/UPDATE/DELETE逆序补偿。业务行恢复和原root/子日志删除在同一控制事务，脏数据/坏片/取消/期限不发布部分恢复，控制COMMIT可能发送后保留原身份及未知，不自动重放原业务。原对象按末级包独立落位且仍PARTIAL；Normal成功原路径删除日志，Consumed不等于所有旧尝试已围栏或通用RM Applied/NotApplied。

最终双Rust各333普通/示例（327+6）、各16实库测试（13补偿+3原包提交故障）、各24原Java观察/10原生输入/7原源编译/2实际JVM清理负控通过。120封存测试程序、12个实际RM恢复子进程、150实际依赖集合均核对；185候选归档未升级。60实际wire副本负控补齐原ACK字节、BEGIN与所有客户端命令、提前prepare后执行的边界及重置命令；6角色负控、七列Undo成功删除/失败保留与另一分支sentinel核验通过。三个证据门禁P2修复后终核无剩余可确认P1/P2；8资格容器及全部27次尝试容器均确认不存在，D数据保留。此前每版63其他DB检查明确继承，未作本轮执行。

源码对应范围仍不包括完整原manager/JUnit、任意历史Java有损tinyint日志或全部JDBC类型/投影/压缩。正式资源适配器/TC所有权、终态记录、所有旧尝试围栏、第一阶段业务/Undo原子COMMIT、原TC覆盖/bin文本、生产TLS/多节点/GC与原全部后续Phase继续；WP05原14项仍0/14、WP06及完整V1保持OPEN，外部TC风险未接受，共享锁仍暂缓。


## 2026-11-17：受管理 MySQL RM 回滚恢复

- 沿用 WP05/ADR-0008，已接通成功注册回执、独立版本恢复表、持久方向及补偿/Undo清理/终态同事务提交；没有公开一阶段业务 COMMIT。
- 修复原围栏对大小写/重音 XID 别名误认；两项真实 RED 与最终双版本当前源码现场均归档。
- Rust1.99/1.85 各336普通测试、64实际harness、1文档测试、38次数据库执行（36种角色）；新MySQL子进程各1，既有RM子进程各6；92原包负控与6角色负控拒绝。实际编译150包/185候选不变。
- 正式目录：`docs/phase-2/evidence/mysql-rm-recovery-gate/run_20261117_125220_94d33cc1`；SHA256 `76eb459a59986e96d29fc51cfa21d4029f9b969014f6f76326f3b69b5a9310e5`；7378输入/7585成员，独立复核无未解决P1/P2。
- 本组件53个历史记录容器均实际确认已不存在；4次源快照失败在建库前，转D保留；全部D数据保留。相同证据仅共享同字节硬链接，原摘要不变。
- [验收记录](phase-2/evidence/mysql-rm-recovery-gate/README.md)及[阶段状态](phase-2/wp05-mysql-rm-recovery.json)已更新。
- 下一步为一阶段 SQL+Undo 与正式TC归属/注册未知恢复交接；提交方向、TC安全、WP05/WP06/V1、完整Java对象/测试/资产迁移仍OPEN，原Java24方法观察仅继承未新跑。外部TC风险未接受，共享锁继续暂缓。


## 2026-11-17：注册未知后的正式回调归属交接

- 沿用 WP05/ADR-0008，新增仅正式回调入口可构造的身份能力；先持久 Pending，再持久回调来源/原方向，随后进入原回滚恢复。公共手工分派不能登记未知分支；调用方在注册未知后主动回滚本地业务。
- 显式 002 扩展迁移保留旧行；已知 None 为 origin=1，原回调缺少锁键信息为 origin=2。迟到真实回执被 CallbackObserved 拦截；正常 None 回执可幂等登记。实际旧程序拒绝扩展表且保留 Pending，新子进程随后恢复。
- Rust1.99/1.85 各339普通测试、68实际harness、2文档测试；每版22次实际数据库执行（18 native含11旧回归＋3 COMMIT故障＋1固定原JavaTC现场）。原包118负控、角色6负控均拒绝；185候选及150实际编译依赖未升级。新编译的是精确资源认证夹具，没有执行原JUnit。
- [最终验收](phase-2/evidence/mysql-callback-ownership-gate/README.md)及[机器状态](phase-2/wp05-mysql-callback-ownership.json)已发布；root run_20261117_145639_40c3447e，catalog SHA256 07435a2698a06f42ed3e9e28c70ca007dbdc915476f9b138ec1114b24bed5db6，11983输入/12188成员。只读独立终核无未解决P1/P2，三处原包验证器P2修复后均有针对性拒绝证据。
- 六个最终自有数据库实例及全部27个已记录容器确认不存在；8组Java运行停止，D盘数据保留。旧24个Java方法观察仍为继承；本次只新跑两个工具链各一个真实TC现场。
- 当前仅完成本回调交接组件。一阶段业务COMMIT/业务Undo原子提交、所有旧尝试围栏、提交方向、原TC锁覆盖、完整WP05/WP06/V1、完整Java对象/测试/资产、生产TLS/多节点/GC仍OPEN；外部TC风险未接受，共享锁继续暂缓。下一步推进一阶段事务准备和TC锁覆盖缺口处理，保持公开COMMIT门禁。


## 2026-11-17：一阶段事务准备交接

- 延续 WP05/ADR-0008；新增消耗原事务的准备接口，实际镜像生成锁键、正式单次注册、独立归属登记、原物理业务事务 Normal Undo 写入。准备对象仅允许只读检查及回滚，错误保留真实成功回执，不重放业务或补造分支编号。
- 空／零行不注册；资源与身份校验、poison／已封存、拒绝／注册未知、早到回调、Guard 竞争、取消／绝对期限及真实归属／Undo 锁等待均有现场证据。三种独立登记 COMMIT 故障保留已知回执，后续原回调完成归属及回滚。
- Rust1.99/1.85 各339普通测试、71实际harness、3文档测试；各33次数据库执行（10准备native／21具体观测＋18回调回归含11旧受管理回归＋1原包同事务＋3登记COMMIT故障＋1固定原JavaTC）。172原包及6角色负控拒绝；185候选／150实际编译依赖未升级。原JUnit未执行。
- [最终验收](phase-2/evidence/mysql-first-phase-preparation-gate/README.md)及[机器状态](phase-2/wp05-mysql-first-phase-preparation.json)已发布；root run_20261117_165622_ee38f20f，catalog SHA256 2d705af619ab9bd0ea68eb3c22ed2eaffc10f2f68086dc313a735a188facba03，11994输入／12205成员。独立终核无未解决P1/P2，跨连接归属COMMIT确认先于补偿BEGIN的验证器P2已修复并有三类目标拒绝证据。API E0599仅标记编译RED，真实期限分类失败诊断另行保留。
- 临时键表CREATE／DROP不可回滚的1751／1752警告严格限定并由另一原mysql客户端复现；业务／Undo始终原事务且实际回滚，不据此声称DDL原子性。14最终数据库及全部26个记录容器确认不存在，3组Java运行停止；D盘现场字节保留。
- 当前完成本准备组件；公开业务COMMIT、业务／Undo原子提交及未知提交恢复、所有旧尝试围栏、提交方向、原TC锁覆盖、完整WP05／WP06／V1、完整原Java对象／测试／资产、生产TLS／多节点／GC仍OPEN。外部TC风险未接受，共享锁继续暂缓；后续推进提交前仲裁、提交方向与原TC排他锁覆盖修复。


## 2026-11-17：原业务事务提交前仲裁

- 延续 WP05/ADR-0008；新增消费式提交前检查及不可解封的持锁作用域。恢复表精确校验、原成功回执／来源／身份／锁键／未绑定方向及完整 Normal root／子片字节核对均使用原业务事务；不新建登记、不变更方向、不提交业务。失败／期限／取消消耗原作用域并保留真实回执。
- 独立引擎观察实际证明业务／Undo／恢复行三组RECORD锁共用ENGINE_TRANSACTION_ID、THREAD_ID和实际连接；另一事务在原回滚前确实等待。两种回调竞争先后、死锁受害者整体拒绝、10类状态／schema拒绝、实际1.5秒期限／取消、Drop、空／零行及旧11列／迁移version1共21个具体观测通过。native回调为受控TCP，与原Java现场区分。
- 双Rust各339普通、73实际harness、4文档测试；各37次DB执行（8仲裁＋10准备回归＋18回调／受管理回归＋1固定原JavaTC）。本轮30原Java原包副本、6角色及106必需成员缺失负控拒绝。185候选／150实际编译依赖未升级；原JUnit未执行。
- [最终验收](phase-2/evidence/mysql-commit-preconditions-gate/README.md)及[机器状态](phase-2/wp05-mysql-commit-preconditions.json)已发布；root run_20261117_174713_97b962fb，catalog SHA256 6e8267381250dba51f680a162287cf7dbd62b32b46f40cc23bf085ce375b708d，12008输入／12224成员。验收器遗漏消费成员入catalog的P2修复后，独立重放106项均拒绝；最终全量只读审查无剩余确认P1/P2。
- 最终8实例及全部12次尝试容器不存在，2组原Java运行停止，D盘现场保留。已停普通数据文件透明压缩前后摘要一致；已停用编译缓存迁往E并逐项核对原字节，未清除业务现场。前序原MySQL报文及三登记COMMIT故障仅继承，没有计成本轮执行；API E0599仅标记编译RED，夹具与1412诊断不称生产RED→GREEN。
- 当前仅完成本仲裁组件。公开业务COMMIT、业务Undo原子提交／未知提交恢复、任意旧尝试围栏、二阶段提交方向、原TC排他锁覆盖、完整WP05／WP06／V1、完整Java源／JUnit／资产迁移、生产TLS／多节点／GC仍OPEN。外部TC风险未接受，共享锁继续暂缓；下一步推进提交方向及完整提交链路所需门槛。


## 2026-11-17：正式提交回调的持久清理

- 显式003能力迁移、正式回调持久授权、同事务原日志清理／终态、提交未知后的原身份恢复已按首期契约验证；不提供公开业务COMMIT。
- 双Rust各339普通／77实际harness／4文档测试，各53次DB执行，新增native38个具体观察。原包反例244、角色反例10、成员缺失反例574全部拒绝；185依赖候选／150实际编译闭包未升级。
- [最终验收](phase-2/evidence/mysql-commit-direction-gate/README.md)及[机器状态](phase-2/wp05-mysql-commit-direction.json)已发布；root run_20261117_235911_9cb6c3df，catalog d4b415c88e994de03ae546968c9343594728663617132f1b5038bc499bccdc98，24067输入／24293成员；独立只读终核无剩余确认P1/P2。
- 原业务COMMIT／未知业务提交恢复、任意旧尝试围栏、原TC排他锁覆盖、完整AsyncWorker／Java源测试资产迁移、WP05／WP06／V1、生产TLS／多节点／GC仍OPEN；共享锁暂缓，外部TC风险未接受。


## 2026-11-18：原Java JUnit执行基线

- 固定cba73e源的4269个Git原始blob逐字节导出，原POM reactor test-compile与JDK17原JUnit引擎实际运行；仅跳过格式化apply保留原字节，原POM/断言/压力参数不改。876POM+403Maven JAR共1279项Central校验一致；189测试/7原资源保留。
- Java1288具体案例通过、39按原JDK17条件禁用；1135方法完整绑定为1096已执行/39禁用，1327案例身份保留，37事件/进程/方法分母反例拒绝。6JVM均停止，原3轮×200线程/30s压力与D home/tmp/hosts、H2清理已核；子JVM不宣称继承父SecurityManager沙箱。
- [验收报告](phase-2/evidence/source-junit-gate/README.md)与[机器状态](phase-2/wp05-source-junit.json)已发布：root run_20261118_013909_5a5c9888，catalog 373012b628b4e74b683c6c2160cca550fdde19b4e48d8d914db8dfba3d37d5a9，8536输入/8546成员。独立只读终核无确认P1/P2，容器生命周期遗漏P2已有针对性拒绝。旧Windows CRLF候选/守卫JMX环境失败留诊断，正式literal账本重算。
- Rust代码/Cargo350文件集与上一root字节不变，双版339/77/4严格继承未重跑；1135原方法与1327案例Rust仍全部MISSING/NOT_RUN，0差分MATCH。原Surefire test/其它JDK/39禁用body及全部Java对象迁移、公开业务COMMIT、TC排他锁安全、WP05/WP06/V1仍OPEN。共享锁暂缓、外部TC风险未接受；下一步据本账本迁移全部sqlparser-core原对象与案例。


## 2026-11-18：SQL parser core基础值对象首批迁移

- 在完整31对象／76源案例清单下新增11真实对象；9 IMPLEMENTED、2因Java孤立代理String域PARTIAL、20 MISSING，余53源案例继续实施。
- 原Java76测试新跑通过，双Rust23源案例逐项MATCH、88常量匹配；各369普通／81 harness／4 doc通过、143 ignored，6运行时mutation／4差分／13JUnit反例拒绝。338原Rust文件及185 registry依赖闭包不变，DB现场本轮不重跑。
- [首批验收](phase-2/evidence/sqlparser-core-values-gate/README.md)、[当前状态](phase-2/wp05-sqlparser-core-values.json)。root run_20261118_022222_bb4f2677，catalog b0056e0927ef6a565c07f7c39ee49b2cc7fed58ad77bc08bf47bec7acec989cd。
- 上一原测试基线的0 MATCH已推进为23 MATCH；两个选定源模块仍1112方法／1304具体案例未迁移。完整core、公开业务COMMIT、原TC安全、WP05/WP06/V1继续OPEN，共享锁延期、外部TC风险未接受。


## 2026-11-18：列元数据与无损UTF-16依赖阶段

- 新增ColumnMeta及全部18源案例，消除两个旧对象UTF16 PARTIAL缺口；当前12 IMPLEMENTED／19 MISSING，累计41源MATCH／35待迁移。
- 两Rust各406普通／85 harness／143 ignored／4doc；21Java补充观察MATCH，5变异共8行为失败，8比较／13JUnit反例拒绝。两项重入P2与ASCII长s遗漏均有真实RED并修复。
- [阶段验收](phase-2/evidence/sqlparser-column-gate/README.md)与[机器状态](phase-2/wp05-sqlparser-column.json)：root run_20261118_030159_4362448c，catalog 1e54b372ce3454028d650bb7d46e72f25a3a4d42c724441d1ec04756ace213da，483成员／422输入。
- 完整core、选定源模块1094方法／1286案例、公开业务COMMIT、TC安全、WP05/WP06/V1继续OPEN；共享锁延期，外部TC风险未接受。


## 2026-11-18：索引元数据与动态异常传播

- IndexMeta新增16原案例，当前13 IMPLEMENTED/18 MISSING，累计57源MATCH/19待迁移。动态回调Result保留原异常。
- 两工具链各434测试、89 harness、4doc；143 ignored。165计算观察MATCH，4身份值只计输入，9行为mutant/8比较反例拒绝。
- [验收证据](phase-2/evidence/sqlparser-index-gate/README.md)与[机器状态](phase-2/wp05-sqlparser-index.json)：run_20261118_042124_511b04b6，catalog 6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774。
- 下步TableMeta19原案例及剩余18对象；无新DB认证，TC安全和全core/WP05/WP06/V1继续OPEN。


## 2026-11-18：TableMeta主键集合依赖

- 固定JDK17比较器及主键红黑树实现，原非传递比较器导致排序Vec偏差的P2有实际RED并修复。
- 23,370新计算观察MATCH，2,799映射只计输入；两工具链各437测试/90 harness/4doc，143 ignored，7运行时mutant/8比较反例拒绝。
- [证据](phase-2/evidence/sqlparser-table-prerequisites-gate/README.md)；[机器状态](phase-2/wp05-sqlparser-table-prerequisites.json)。
- 未将依赖实现算作TableMeta完成：源对象仍13/31、源案例仍57/76；继续默认Locale映射、TableMeta19原案例及剩余18对象。


## 2026-11-18：TableMeta Locale上下文依赖

- JDK17上下文大小写及随机词边界/泰语词典实现；12,384组三字段MATCH，5,004属性及3输入文件不计结果。
- 双工具链各440测试/91 harness/4doc，143 ignored；8运行失败＋1未分类存活变体，12比较负控拒绝。有限353,922次搜索不是等价证明。
- [证据](phase-2/evidence/sqlparser-locale-gate/README.md)及[状态](phase-2/wp05-sqlparser-locale.json)。源码13/31、案例57/76仍未增加，继续TableMeta集成。


## 2026-11-18：TableMeta首期实现与原76案例差分

- 原76案例双工具链MATCH；原Java76本轮新运行，283主键顺序/12表边界匹配。共享映射、重入、异常继承已实现。
- Rust1.99/1.85各471测试/95 harness/4doc，143 ignored；验收包131通过/0忽略；6运行变体拒绝、12比较负控拒绝。
- [证据](phase-2/evidence/sqlparser-table-gate/README.md)和[机器状态](phase-2/wp05-sqlparser-table.json)。对象13完整/1部分/17缺失；默认Locale、SPI和完整Map继续实施，全部交付保持OPEN。


## 2026-11-18 SQL Server 表元数据

已实现 IDENTITY 标志和 TableMeta 继承分派。两套工具链各 472 项普通测试及 4 项文档测试通过，76 个原案例重新采集后保持 MATCH。SQL parser core 当前有 13 个完整对象、2 个部分实现对象和 16 个缺失对象。 [验收记录](phase-2/sqlparser_core/verification/sql-server-table-result.json)


## 2026-11-18 标识符转义

已移植 EscapeHandler 的 UTF-16 默认转义规则。397 条 Java 观察与 76 个原案例均匹配。两套 Rust 工具链各通过 133 项验收包测试。Rust 1.85 全量回归仍有 TM 注册和 RM 重复回调失败。方言 SPI 与主键转义接入继续实施。 [验收记录](phase-2/sqlparser_core/verification/escape-handler-result.json)

Rust 1.85 两项失败用例隔离复测通过，随后串行全量通过 473 项普通测试和 4 项文档测试。早期失败日志保留。 [复测证据](phase-2/evidence/sqlparser-escape-handler/msrv-follow-up.json)


## 2026-11-18 MySQL 主键转义

已接通 TableMeta 到 MySQL 方言，280 条原 Java 观察和 76 个原案例均匹配。现有对象 13 完整、5 部分实现、13 缺失，其他方言及自动 SPI 继续实施。 [验收记录](phase-2/sqlparser_core/verification/mysql-escape-pk-result.json)


## 2026-11-18 MySQL 转义 SPI

已实现链接发现、优先级选择及并发缓存。12 条原 SPI 观察与 280 条 MySQL 转义观察匹配。 [验收记录](phase-2/sqlparser_core/verification/mysql-escape-spi-result.json)


## 2026-11-18 十个生产转义方言

补齐原服务清单中的 9 个方言，10 个方言均经自动 SPI 接通主键转义。5,335 条 Java 观察及 76 个原案例匹配。 [验收记录](phase-2/sqlparser_core/verification/escape-dialects-result.json)


## 2026-11-18 表元数据实时视图

已接入实时 Entry/Values、脱离条目的身份保留和键快照。35 条原 Java 观察、76 个原案例匹配。另有一项已实测的比较回调重入差异，继续追踪。 [验收记录](phase-2/sqlparser_core/verification/lower-case-map-views-result.json)


## 2026-11-18 表元数据 Map 默认方法

已接入 11 个默认入口，40 条原 Java 观察和 76 个原案例匹配。集合结构重入、批量写入和其余 Java 集合接口继续实施。 [验收记录](phase-2/sqlparser_core/verification/lower-case-map-defaults-result.json)


## 2026-11-18 表元数据 Map 批量写入与浅复制

已接通两遍 putAll、复制构造及 Java clone 行为，25 条原 Java 观察和 76 个原案例匹配。批量错误保留部分写入。复制保留值身份及独立条目。 [验收记录](phase-2/sqlparser_core/verification/lower-case-map-copy-result.json)


## 2026-11-18 JDBC URL 类型识别

已实现 DbTypeParser 与 RM 的 JDBC 类型入口。Druid 与 RM 原 Java 各 557 条观察匹配，原 76 个核心案例也保持匹配。 [验收记录](phase-2/sqlparser_core/verification/jdbc-db-type-result.json)


## 2026-11-18 JDBC 驱动类名与资源 ID

64 个驱动前缀经原 Druid 验证，驱动类名与 RM 资源 ID 各 930 条观察匹配。MySQL 和 ClickHouse 缓存重入有 6 条原 Java 观察及修复前后的 Rust 回归。Rust 1.99 和 1.85 各通过 488 项普通测试、4 项文档测试及 76 个原案例差分，143 项环境测试仍忽略。 [验收记录](phase-2/sqlparser_core/verification/jdbc-driver-result.json)

## 2026-11-18 RM JDBC 原始测试逐案对齐

固定版 `JdbcUtilsTest` 的 21 个原始 JUnit 案例通过，8 个 URL 类型与资源 ID 案例与 Rust 实测逐案匹配。两套 Rust 工具链各通过 496 项普通测试和 4 项文档测试，143 项环境测试仍忽略。两个隔离行为变异触发了对应原断言。普通 JDBC 与 XA 的 17 条原 Java 初始化轨迹已采集，剩余 13 个 RM JDBC 原始案例仍待迁移。 [验收记录](phase-2/wp05-rm-jdbc-source.json)

## 2026-11-18 RM JDBC 服务发现与单例

RM 接通 Druid 链接服务注册。原 Java 与 Rust 对照验证大写 DRUID 覆盖、最高同名优先级、共享实例、空名异常和缺失名异常。绕过发现或移除服务声明均触发运行时测试失败。RM JDBC 累计 10/21 个原案例匹配，两模块累计 86 个。两套 Rust 工具链各通过 501 项普通测试和 4 项文档测试，143 项环境测试仍忽略。 [验收记录](phase-2/wp05-rm-jdbc-spi.json)

## 2026-11-18 RM JDBC 驱动加载适配边界

新增上下文和默认加载域的回退、异常继承匹配，以及带原异常身份的 SQL 异常包装。17 条原 Java 轨迹和 Rust 宿主输入差分匹配，两个隔离变异分别触发错误回退和异常子类型断言。双工具链各通过 505 项普通测试和 4 项文档测试，143 项环境测试仍忽略。两模块源案例累计 86 个匹配，实际 JVM 驱动宿主与 JDBC/XA 数据源初始化继续实施。 [验收记录](phase-2/wp05-rm-jdbc-load-driver.json)

## 2026-11-18 RM JDBC 与 XA 资源初始化

18 条原 Java 轨迹与 Rust 适配实现匹配，覆盖 JDBC 和 XA 关闭顺序、部分初始化状态、异常包装及注册条件。探测失败不写缓存。独立运行时变异验证 XA 在失败后仍须关闭外层连接，普通连接须保留 suppressed。Rust 1.99 和 1.85 各通过 509 项普通测试及 4 项文档测试，143 项环境测试仍忽略。具体 JVM/数据库宿主及原案例迁移继续实施。 [验收记录](phase-2/wp05-rm-jdbc-init.json)

## 2026-11-18 WP05 一阶段持久见证

004 显式迁移允许在原业务事务核对恢复行和 Normal Undo 后暂存一阶段见证。固定 MySQL 8.4.13 的独立观察者实际等待恢复行锁，回滚后见证、业务和 Undo 均不可见。两个 Rust 工具链各通过 10 项实际 MySQL 现场用例，包括管理事务提交的正样本和受控 TCP 回调后的持久见证读取。省略候选写入的变异触发真实断言失败，自有容器已清理。公开业务提交、原 Java TC 排他锁覆盖和旧尝试围栏继续实施。 [验收记录](phase-2/wp05-mysql-first-phase-witness.json)

## 2026-11-18 WP05 排他锁行语法校验

正式 AT 准备在发送登记前校验原编码行。空串、Java 空白、含空行的多行键、分号截断以及多字节二进制列表在真实 MySQL 镜像场景中被拒绝。本轮分别运行固定 Java TC 的原收集方法及受控 TCP 对端，后者只收到允许的登记，原键字节保持。移除分号校验的变异触发真实失败。Rust 1.99 和 1.85 各通过 11 项实际 MySQL 用例，另有 509 项普通测试和 4 项文档测试。完整原 TC 网络、实际锁表和公开业务提交验收继续实施。 [验收记录](phase-2/wp05-tc-lock-registration-guard.json)

上一轮一阶段持久见证已完成独立只读复审。 [复审记录](phase-2/evidence/mysql-first-phase-witness/readonly-review-20261118.json)

## 2026-11-18 WP05 正式 RM 排他锁现场联试

双工具链均经正式 Rust TM/RM、固定原 Java TC 和实际 MySQL 8.4.13 验证了普通整数键的锁归属、同键分支登记冲突和回滚后的锁释放。空串及分号键各有一次实际写入后在发送登记前被拒绝。独立 SQL 与原始帧共同核对 XID、BranchID 和实际锁行。另通过 Docker 创建成功但结果丢失的注入，验证了自有容器的失败清理。两套工作区各通过 509 项普通测试及 4 项文档测试，另有 147 项环境测试默认跳过。公开业务提交和完整 TC 锁安全门槛仍待验收。 [验证记录](phase-2/wp05-tc-lock-native-registration.json)

## 2026-11-18 WP05 原 TC DB 锁字段上限和排序规则

双工具链各用正式 Rust AT 路径向固定 Java TC 登记八次。MySQL 严格模式下，`pk`、`table_name`、`row_key` 分别在 36、32、128 字符成功，超一字符的三次请求均被明确拒绝。业务表可同时写入的 `Cafe` 和 `café` 在 TC 锁存储中发生竞争。锁查询、原回包、八次全局回滚、终态恢复与 Undo 围栏均已核对。两套全工作区各通过 509 项常规测试及 4 项文档测试，147 项环境测试默认跳过。资源别名、复合键碰撞、公开业务提交及完整 TC 安全门槛仍待验收。 [验证记录](phase-2/wp05-tc-lock-storage-boundaries.json)

## 2026-11-18 WP05 TC 锁身份边界

双工具链均实测同一 MySQL 行经不同 JDBC 资源 ID 可在原 TC 同时持有两条锁，同 ID 对照登记冲突。实际可并存的两个复合主键元组生成相同锁键，原 TC 因此拒绝其中一次登记，另一不同锁键对照成功。各跑完六次全局回滚，锁表清空，四个成功分支各保留一条 Undo 围栏及终态恢复记录。完整物理排他覆盖、公开业务提交和原 WP05、WP06、V1 验收继续。 [验证记录](phase-2/wp05-tc-lock-identity-boundaries.json)

## 2026-11-18 WP05 受管理回滚后的晚到 Undo 围栏

004 恢复配置下的受管理回滚会在消费 Normal Undo 后保留同一原 XID 和分支的 GlobalFinished。补偿、日志删除、围栏和终态共用一次控制事务。原失败用例证实旧事务可重新写入，修复后真实 MySQL 返回 1062。双工具链各运行 11 个实际 MySQL 仲裁用例，独立 SQL 核对重启前后围栏、终态和原提交见证一致。历史终态核验、公开业务提交和整体门槛仍待完成。 [验证记录](phase-2/wp05-mysql-managed-rollback-fence.json)

## 2026-11-19 WP05 受管理回滚控制提交故障

双工具链的六次 MySQL 实跑覆盖真实提交回包丢失、COMMIT 未送达和超期限回包。首回均保持可重试和 Pending。已提交的两个场景在重试时读取终态，未送达场景由重试执行唯一的补偿提交。最终围栏、原见证及回滚终态经独立 SQL 与 MySQL 重启核对。今天用现版验证器重验了原始包，Docker Linux 引擎 HTTP 500 使新的容器复跑停在启动前。 [验证记录](phase-2/wp05-mysql-managed-rollback-fault.json)

## 2026-11-19 WP05 原始 JDBC 驱动用例

固定版 `JdbcUtilsTest` 21 项重新执行通过。四个原始驱动案例通过实际 JVM 原语和原 H2 驱动与 Rust 正式加载入口逐案对照，双工具链结果完全一致。去掉 H2 JAR 确认原成功案例失败。RM JDBC 原案例累计 14/21 匹配，两模块合计 90 个。两版各通过 509 项常规测试和 4 项文档测试。 [逐案验证](phase-2/wp05-rm-jdbc-driver-jvm.json)

## 2026-11-19 WP05 JdbcUtilsTest 源案例迁移

两套工具链新增匹配七个原 JDBC/XA 初始化用例，保留模拟连接输入并验证实际 H2/MySQL 驱动、字段写入、资源登记、异常因果链和关闭行为。重跑四个驱动用例，连同此前十项核对同一组 21 个原 JUnit ID。两个缺驱动负控各导致对应成功用例失败。两模块累计 97 个原案例匹配，完整源迁移与 WP05 其余验收继续。 [逐案验证](phase-2/wp05-rm-jdbc-init-source.json)

## 2026-11-19 WP05 Undo 镜像的主键匹配

修复右侧重复主键被重复匹配的问题。两套工具链验证两侧重复均不相等、合法复合元组重排仍相等。原 Java 两个比较测试类实际通过 12 个 JUnit 用例，追加探针定位了重复方向差异和复合键拼接覆盖。这批原用例仍待逐案迁移，累计匹配保持 97。 [验证记录](phase-2/wp05-mysql-record-comparison.json)

## 2026-11-19 WP05 DataCompareUtils 逐案迁移

原对象入口已通过固定 Java 两个测试类的 12 个用例、45 项断言和六项重复键补充观察。两套 Rust 工具链均覆盖 Fastjson 字段原地转换、可空比较及原字符串键映射，累计匹配 105 个原始测试方法及 109 个具体用例，尚余 1030 个方法及 1218 个具体用例。MySQL 回滚继续使用原生主键一对一比较。 [逐案验证](phase-2/wp05-mysql-record-compare-source.json)

原对象比较的两个边界修复已通过固定 Java 和两套 Rust 工具链验证。补充观察增至 12 项，45 项原断言继续逐值匹配。每套工具链通过 522 个工作区测试、4 个文档测试和两项物理回滚比较回归。 [边界修复证据](phase-2/wp05-mysql-record-compare-boundaries.json)

Fastjson 行级兼容入口已对照五项原 Java 观察。两套 Rust 各通过 525 个工作区测试与 4 个文档测试，实际削弱序列化器分派及列遍历顺序分别触发目标失败。 [行级验证](phase-2/wp05-mysql-fastjson-row-compare.json)

Fastjson 可变记录入口已覆盖空值和左主键元数据顺序。两套工具链各通过 527 个工作区测试、4 个文档测试和两个物理回滚回归；跳过空记录返回及误用右侧元数据的两个实际变体均触发对应失败。 [记录级验证](phase-2/wp05-mysql-fastjson-record-compare.json)

Fastjson JDBC UTC 宿主已通过三个原时间用例及 24 项 JDK 原语比较。两套工具链各通过 531 个工作区测试、4 个文档测试和两个物理回滚回归；移除跨日进位及截断纳秒的变体均触发预期失败。 [UTC JDBC 验证](phase-2/wp05-mysql-fastjson-jdbc-utc.json)

12 个原 DataCompareUtils 用例的 Rust 执行已移除 JVM 日期时间夹具。新增 29 项 LocalDateTime 原语观察与固定 JDK 逐值匹配，两套 Rust 各通过 533 个工作区测试、4 个文档测试与两个物理回滚回归。放宽四月的日期上限和丢失纳秒的两个变体均触发预期失败。 [LocalDateTime 验证](phase-2/wp05-mysql-fastjson-local-datetime.json)

Fastjson 原 Undo 载荷已完成有界解码和物理元数据绑定。固定 Java 和两套 Rust 匹配 36 个字段观察；每套工具链通过 535 个工作区测试和 4 个文档测试。伪造内部字面量标记与忽略 JDBC 类型两个削弱变体都触发预期失败。 [Fastjson Undo 解码验收](phase-2/wp05-fastjson-undo-native.json)

Fastjson Undo 新增元数据读取前预检。两套工具链各通过 537 个工作区测试和 4 个文档测试，独立复审发现的缺键 panic 和归一化预算绕过均已复现并修复。跳过分支身份、允许跨库表名和移除归一化字节检查的三个变体分别触发预期失败。 [预检验收](phase-2/wp05-fastjson-undo-preflight.json)

Fastjson 已接到内部 MySQL 控制事务验收入口。Rust 1.99 与 1.85 各通过 538 个工作区测试与 4 个文档测试。忽略根行序列化器和误用 Jackson 解码的两个变体触发预期失败。Docker API 1.56 和 1.44 均返回 HTTP 500，真实测试已编译，仍待执行。 [控制事务接线证据](phase-2/wp05-fastjson-undo-control-transaction.json)

Fastjson 默认内容继承用例已逐案匹配。原 Java 七例重新执行通过，两套 Rust 各通过 540 个工作区测试与 4 个文档测试。全项目累计 110 个具体源用例和 105 个已完整匹配的方法，尚余 1217 个具体用例及 1030 个方法。 [逐案验收](phase-2/wp05-fastjson-undo-default-source.json)

Fastjson testWriteClassName 原例逐字节匹配。两套 Rust 各通过 542 个工作区测试与 4 个文档测试；去除 Long 标记及 BigDecimal 尾点的变体均触发预期失败。源迁移累计 111 个具体用例、106 个已完整匹配的方法。 [类型保留逐案验收](phase-2/wp05-fastjson-undo-write-class-source.json)

Fastjson 已完成三个原 SerialArray 用例。两套 Rust 各通过 545 个工作区测试和 4 个文档测试。源迁移累计 114 个具体用例、109 个已完整匹配的方法。Fastjson 原 JUnit 仍余继承的 testEncodeAndDecode 与 testPerformance。 [数组逐案验收](phase-2/wp05-fastjson-undo-serial-array-source.json)

Fastjson 原测试类七个具体用例全部匹配。两套 Rust 各通过 545 个普通工作区测试及 4 个文档测试，另在专用验收执行真实 H2 输入的两个具体用例。源迁移累计 116 个具体用例、109 个已完整匹配的方法。 [H2 编解码逐案验收](phase-2/wp05-fastjson-undo-h2-source.json)


Jackson 已匹配三个继承原用例，还剩六个具体用例。两套 Rust 各通过 549 个普通工作区测试、4 个文档测试及另行启用的两个 H2 原用例，交换前后镜像和移除 Long 标签的变体均触发预期失败。源迁移累计 119 个具体用例、109 个已完整匹配的方法。 [Jackson 原用例验收](phase-2/wp05-jackson-undo-h2-source.json)


Jackson 的两个原 SerialArray 用例已逐字节匹配，原测试类累计 5/9。两套 Rust 各通过 551 个普通工作区测试和 4 个文档测试，倒置元素顺序和替换空位的变体均触发预期失败。全项目累计匹配 121 个具体源用例、111 个完整声明方法。 [Jackson 数组逐案验收](phase-2/wp05-jackson-undo-serial-array-source.json)


Jackson 独立 Field encode 原用例已匹配，原类累计 6/9。两套 Rust 各通过 554 个普通工作区测试和 4 个文档测试。源迁移累计匹配 122 个具体用例和 112 个完整方法。 [独立 Field 原用例验收](phase-2/wp05-jackson-undo-field-source.json)


Jackson 两个达梦原用例已匹配，原类累计 8/9。两套 Rust 各通过 557 个普通工作区测试和 4 个文档测试。源迁移累计匹配 124 个具体用例和 114 个完整方法。 [达梦逐案验收](phase-2/wp05-jackson-undo-dmdb-source.json)


Jackson 原测试类九个具体用例全部匹配。两套 Rust 各通过 558 个普通工作区测试与 4 个文档测试，另外重新执行了真实 H2 的两个原用例。全项目累计匹配 125 个具体源用例和 114 个完整方法。Fastjson2 五个原用例的 Java JSONB 已捕获。 [Jackson 原类验收](phase-2/wp05-jackson-undo-branch-timestamp-source.json)


Fastjson2 五个原用例均已匹配完整 JSONB。两套 Rust 各通过 563 个普通工作区测试、4 个文档测试及另行启用的两个 H2 原用例。全项目累计匹配 130 个具体用例和 116 个完整方法。Protostuff 的四个 Java 原用例已完成二进制观测。 [Fastjson2 验收](phase-2/wp05-fastjson2-undo-source.json)


Protostuff 四个原用例均已匹配完整二进制。两套 Rust 各通过 569 个普通工作区测试、4 个文档测试及另行启用的两个 H2 原用例。全项目累计匹配 134 个具体用例和 116 个完整方法。固定 Java 已捕获四个 Kryo 原用例。 [Protostuff 验收](phase-2/wp05-protostuff-undo-source.json)


Kryo 四个原用例均已匹配。两套 Rust 各通过 575 个普通工作区测试、4 个文档测试及另行启用的 H2 原用例。全项目累计匹配 138 个具体用例和 117 个完整方法。Fory 的三个原用例已完成 Java 二进制观测。 [Kryo 验收](phase-2/wp05-kryo-undo-source.json)


Fory 三个原用例均已匹配，六个 UndoLogParser 原测试类累计 32 个具体用例全部匹配。两套 Rust 各通过 579 个普通工作区测试、4 个文档测试及另行启用的两个 H2 原用例。全项目累计匹配 141 个具体用例和 120 个完整方法。 [Fory 验收与原类覆盖](phase-2/wp05-fory-undo-source.json)

Fastjson2 的两份固定 Java JSONB 原载荷均通过 Rust 逐字节往返和 MySQL 物理元数据离线绑定。每份各验证三个 SQL 日志、六份镜像和 36 个字段，补齐无符号 BIGINT、精确 DECIMAL(65,30) 与二进制。Rust 1.99 与 1.85 各通过 587 个普通工作区测试、4 个文档测试和两个用固定 H2 观测重跑的原用例。原用例迁移累计仍为 141 个具体用例与 120 个完整方法。Docker Engine API 返回 HTTP 500，真实 MySQL 回滚和生产分派仍待验收。 [Fastjson2 物理绑定验证](phase-2/wp05-fastjson2-undo-physical.json)

Fastjson2 已接入 MySQL 内部控制事务的精确格式分派。锁定根行须声明 `fastjson2/NONE`，同一预检文档供表查询及全量物理绑定。两套 Rust 各通过 588 个普通工作区测试、4 个文档测试和严格 Clippy。原 Java JSONB 分片回滚测试已在两套工具链编译，Docker Engine HTTP 500 阻止现场执行，WP05 与生产接入仍开放。 [Fastjson2 控制事务验证](phase-2/wp05-fastjson2-undo-control-transaction.json)

Undo 解析器工厂、SPI 和分支对象的三个原 Java 用例已逐案匹配。六种内置解析器共享 SPI 单例，工厂保留默认 Jackson、`fury` 别名与 FST 拒绝。分支原用例经工厂生成的 JSON 与固定 Java 逐字节一致。两套 Rust 各通过 592 个普通工作区测试、4 个文档测试及严格 Clippy。累计匹配 144 个具体源用例和 123 个完整方法。Docker Engine 仍返回 HTTP 500，现场 MySQL 验收待执行。 [工厂与 SPI 原用例验证](phase-2/wp05-undo-parser-selection-source.json)

TableRecordsTest 的六个原 Java 用例均已运行，前五例已与 Rust 逐案匹配。固定 Druid mock 的八个字段以及两行主键数量和值均相符。第六例的 Java OffsetDateTime 及序列化字节已捕获。Rust 1.99 与 1.85 各通过 597 项普通工作区测试、4 项文档测试及严格 Clippy；累计匹配 149 个原用例和 128 个完整测试方法。 [TableRecords 原用例验证](phase-2/wp05-table-records-source.json)

TableRecordsTest 六个原 Java 用例现已全部与 Rust 匹配。JDBC 2014 的 OffsetDateTime 经默认 UTC Jackson 数值模式逐字节往返，补充 20 条固定 Java 观测。Rust 1.99 与 1.85 各通过 599 项普通工作区测试、4 项文档测试和严格 Clippy。累计匹配 150 个原用例和 129 个完整方法。Docker Engine /version 仍返回 HTTP 500，真实 MySQL 验收保持开放。 [TableRecords OffsetDateTime 验收](phase-2/wp05-table-records-offset-source.json)

WP05 单独 Undo 回滚控制事务现已保留清理前取得的原错误，回滚失败或超时仍返回原身份和故障原因。发送控制 COMMIT 后超期保持结果未知。固定 Java UndoLogManagerBehaviorTest 的 15 例已运行，映射的三个失败场景目前只有 Rust 离线故障证据，原用例完整迁移计数维持 150 例和 129 个完整方法。Rust 1.99 与 1.85 各通过 602 项普通工作区测试、4 项文档测试及严格 Clippy。Docker Engine HTTP 500 仍阻止真实 MySQL 清理验收。 [控制事务失败验证](phase-2/wp05-undo-rollback-failure.json)

WP05 已用实际 SQLx 0.8.6 和回环协议对端验证生产回滚入口。首条连接执行 SHOW GRANTS 收到 SQLSTATE 42000、错误码 1142，随后显式等待 ROLLBACK。两种回滚故障均保留首错、关闭原连接且未发送 COMMIT，容量为 1 的池下一次借用建立第二条连接。Rust 1.99 与 1.85 各通过 604 项普通工作区测试、4 项文档测试及严格 Clippy。真实 InnoDB 事务验收仍待执行。 [SQLx 回环验证](phase-2/wp05-undo-rollback-sqlx-cleanup.json)

WP05 的 Undo 写入现先规划完整分片和父子引用，再执行子片在先、根片在后的插入。固定 Java 原类三例已运行，分片原字节和上下文已对照，原 JDBC 写入及关闭断言仍待补足。Rust 1.99 与 1.85 各通过 607 项普通工作区测试、4 项文档测试及严格 Clippy。Docker Engine HTTP 500，真实 MySQL 分片验收仍待执行。 [分片布局验证](phase-2/wp05-undo-fragment-layout.json)

MySQLUndoLogManagerBehaviorTest 的原分片写入用例已完成 Rust 对照。SQLx 回环验证逐片核对 XID、Normal 状态、载荷和预编译句柄关闭，第二子片写入失败后根片未发送，事务由调用方回滚。Rust 1.99 与 1.85 各通过 609 项普通工作区测试、4 项文档测试及严格 Clippy。累计匹配 151 个原用例和 130 个完整方法。真实 InnoDB 分片写入与原子性验收待执行。 [分片 SQLx 验证](phase-2/wp05-undo-fragment-sqlx.json)

WP05 的 Undo 读取现在要求实际子片集合与根行声明完全一致。SQLx 回环测试证明重复、孤儿和缺失子片均在正文 SELECT 前被拒绝，正常数据按根片和 subId 的声明次序拼接。Rust 1.99 与 1.85 各通过 612 项普通工作区测试、4 项文档测试及严格 Clippy。原 Java 读取用例尚待完成全部断言迁移，累计完整匹配维持 151 个原用例和 130 个方法。Docker Engine HTTP 500，真实 InnoDB 验收仍待执行。 [分片读取回环验证](phase-2/wp05-undo-fragment-read-sqlx.json)

WP05 的 Normal Undo 删除现在在原唯一键查找之外核对 XID 字节，子片同时核对原父号上下文。SQLx 回环确认子片先删、根行最后，第二子片返回零行或错误时停止。三套验证器已回放九份历史线缆记录，并通过新格式的独立验证。Rust 1.99 与 1.85 各通过 614 项普通工作区测试、4 项文档测试及严格 Clippy。Docker Engine HTTP 500，真实 InnoDB 清理和事务验收待执行。 [分片清理证据](phase-2/wp05-undo-fragment-cleanup-sqlx.json)

WP05 托管 RM 补偿控制事务现已在原期限内保留首错并显式清理。SQLx 回环验证 ROLLBACK 报错或未回复时均返回原分支身份、SQLSTATE 42000 和错误码 1142，原连接关闭且未发送 COMMIT。Rust 1.99 与 1.85 各通过 616 项普通工作区测试、4 项文档测试及严格 Clippy。Java 用例匹配仍为 151 例和 130 个完整方法。Docker Engine HTTP 500，真实 InnoDB 与完整 TC 回调验收仍待执行。 [托管回滚证据](phase-2/wp05-managed-rollback-sqlx-cleanup.json)

WP05 的 Undo 读取现接受 Java 定义的空白 `subId`，同时校验原父号下确无子片。固定 JDK 17 的全部 65,536 个 UTF-16 代码单元判定一致，固定原类 3 例重跑通过。SQLx 回环验证根片读取、孤儿子片拒绝和不换行空格拒绝，Rust 1.99 与 1.85 各通过 618 项普通工作区测试、4 项文档测试和严格 Clippy。原读取用例的 JDBC 资源关闭和数值异常因果仍待匹配，完整匹配维持 151 个用例、130 个方法。Docker Engine HTTP 500，真实 InnoDB 验收待执行。 [空白 subId 验证](phase-2/wp05-undo-blank-subid.json)

WP05 的 Undo 分片读取现在保留无效数字和 i64 溢出的解析原因。SQLx 回环逐片验证正文语句关闭，并通过扣留最后一个结果集结束帧确认读取不会提前完成。Rust 1.99 与 1.85 各通过 619 项普通工作区测试、4 项文档测试和严格 Clippy。固定原 Java 类 3 例重跑通过，其中读取用例因尾分隔符和查询形态差异仍为部分匹配。公开 `DbError` 增加 `InvalidUndoSubId`，下游穷尽匹配需包含新变体。Docker Engine HTTP 500，真实 InnoDB 验收待执行。 [数值错误和资源验证](phase-2/wp05-undo-read-resources.json)

WP05 的 Undo 读取支持有无单个尾逗号的子号列表，写入继续保存 Java 格式。子片正文在锁定身份、长度和成员后用一条语句读取，按声明顺序绑定子号及原 XID，消费完整响应后按声明重组。原 `loadsFragmentsWithBoundIdentifiersAndClosesJdbcResources` 已在 SQLx 回环逐案匹配，累计达到 152 个原用例和 131 个完整方法。固定 Java 原类 3 例通过，两套 Rust 各通过 620 项普通工作区测试、4 项文档测试和严格 Clippy，三组隔离负控均触发目标失败。原驱动异常转换用例和真实 InnoDB 验收仍待完成，Docker Engine 返回 HTTP 500。 [固定 Java 读取用例](phase-2/wp05-undo-fragment-read-source.json)

WP05 的 INSERT 和单子片 101 读取已在 SQLx 回环验证 PREPARE 拒绝，保留 SQLSTATE 42000 与错误码 1142，未发送 EXECUTE。INSERT 在等待 PREPARE 时断连，错误链保留脱敏的 EOF 原因。Rust 1.99 和 1.85 各通过 624 项普通工作区测试、4 项文档测试及严格 Clippy。固定原 Java 类 3 例通过，驱动异常用例因按日期删除、空载荷路径及 Java 异常对象身份仍为部分匹配。公开 `DbError::Driver` 增加 `io_source`，下游显式构造及未使用 `..` 的字段匹配需要适配。真实 InnoDB 验收待执行，当前 Docker Engine 返回 HTTP 500。 [驱动诊断验证](phase-2/wp05-undo-driver-prepare.json)

WP05 的独立 Undo 回滚已区分连接池获取期限与整体操作期限。真实 SQLx 回环验证认证拒绝保留 1045/28000，两种期限先后均保留原 XID 与分支号，且在 BEGIN 前停止。两套 Rust 各通过 627 项普通工作区测试、4 项文档测试与严格 Clippy，两个隔离负控均触发预期失败。固定 Java 原类 15 例重新执行通过，连接获取原用例仍待对照明确的 Java 可重试分类和异常对象身份。Docker Engine 返回 HTTP 500，真实 InnoDB 验收待执行。 [连接获取验证](phase-2/wp05-undo-acquisition-sqlx.json)

WP05 托管 RM 已区分连接池获取期限。正式 Rust 回调路径通过受控 TC TCP 对端收到两次同分支回滚请求，SQLx 在来源确认阶段认证失败，两次回包均保留原身份、消息编号和可空头并返回 Java V1 可重试状态 9。MySQL 对端未收到事务 SQL，RM 关闭后重开文件仍读到 Pending。Rust 1.99 与 1.85 各通过 629 项普通测试、4 项文档测试及严格 Clippy，三组隔离负控均触发预期失败。Java 固定原类 15 例通过，原异常对象身份及完整 Java TC 补偿路径仍待对照。Docker Engine 返回 HTTP 500，真实 InnoDB 验收待执行。 [托管回调验证](phase-2/wp05-managed-acquisition-callback.json)

已从 D 盘现有 Ubuntu WSL 启动独立 Docker 引擎，按原摘要验证并运行 MySQL 8.4.13，恢复 WP05 的真实 InnoDB 验证。Rust 1.99、1.85 各通过 24 项现场测试（13 回滚、10 写入、1 补偿连接故障）、629 项普通测试、4 项文档测试和严格 Clippy。回调来源确认及方向绑定后阻断补偿连接，首次保留原 Undo 七列、业务行和 Pending；同一分支的显式重试完成补偿。隔离容器、引擎和回环转发已关闭，原始镜像和验证材料保留在 D 盘。完整 Java TC 链路和 WP05 其他门槛仍待验收。 [真实 MySQL 验证](phase-2/wp05-owned-wsl-mysql-validation.json)

WP05 已在固定 Java TC FILE、正式 Rust TM/RM 和真实 MySQL 8.4.13 上验证补偿连接获取失败后的 TC 自动重试。TM 一次回滚，原 TC 两次同分支回调，回包依次为 9 和 8。首次故障保留业务行及 Undo 七列，重试完成后业务行与 Undo 清零，原分支日志从 Pending 转为 Completed，原 XID 最终查询为 Finished。Rust 1.99 与 1.85 各通过现场用例、629 项普通测试、4 项文档测试及严格 Clippy。固定 Java TC 登记响应丢失回归也通过。测试进程、隔离容器和转发均清理完成。 [固定 Java TC 重试验证](phase-2/wp05-real-java-tc-compensation-retry.json)

WP05 已修复使用二进制排序规则的文本列镜像读取。真实 MySQL 8.4.13 覆盖 `utf8mb4_bin`、`utf8mb4_0900_bin`、`ascii_bin` 和 `utf8mb3_bin`，验证 UPDATE 和 INSERT 镜像、区分大小写的原锁键、空文本与 NULL、真实二进制列及生产 Undo 补偿。固定 Connector/J 8.0.33 独立核对恢复后的字段类型及字节。Rust 1.99 和 1.85 各通过 51 项 MySQL 现场用例、629 项普通工作区测试、4 项文档测试和严格 Clippy。用于补偿验证的已提交业务行和 Undo 由隔离夹具一起准备。排他锁完整覆盖和公开一阶段 COMMIT 仍按原验收跟踪。 [二进制排序规则验证](phase-2/wp05-mysql-bin-text-images.json)

WP05 补上复合文本主键的登记校验。两个及以上文本主键分量内出现下划线时，正式准备在发送 BranchRegister 前拒绝，追加 Undo 继续应用同一规则。真实 MySQL 和固定 Java TC DB 在双 Rust 各验证两组碰撞元组零登记、普通复合键可登记、六个全局事务结束以及实际锁表清空。独立受控对端还验证了含歧义行的多行回滚和单文本加整数的安全混合主键。Rust 1.99 和 1.85 各通过 632 项工作区测试、11 项真实 MySQL 一阶段准备测试、4 项文档测试和严格 Clippy。物理资源别名和 TC 存储排序规则仍待完整验收。 [复合主键登记验证](phase-2/wp05-tc-lock-composite-registration.json)

WP05 新增 MySQL 业务 schema 的可选资源绑定。部署方预置获准的原 JDBC 资源 ID，Rust 在有 Undo 的原业务事务中核对原字节并锁定绑定行。已验证缺表、缺行、大小写不符均零登记，修改绑定行及 DDL 均等待事务回滚，只读账号可锁定读取而写入被 MySQL 拒绝。固定 Java TC DB 联试验证同一 MySQL 经两个端口访问时获准端点正常登记，别名零分支登记，锁表最终清空。Rust 1.99 和 1.85 各通过 11 项 MySQL 现场准备、632 项工作区测试、4 项文档测试与严格 Clippy。所有写入器统一原资源 ID、稳定物理路由及完整 TC 安全门槛仍待验收。 [资源绑定验证](phase-2/wp05-tc-lock-resource-binding.json)

WP05 新增原版 Java TC MySQL 锁表的可选长度预检。Rust 按 utf8mb4 字符数逐行检查资源 ID、表名、主键和组合行键，原锁键字节直接用于登记。固定 Java TC 与 MySQL 8.4.13 现场先复现 37 字符主键触发 FailedStore，再验证启用预检时三种超限均无分支登记帧，边界内四项成功登记。当前实现默认模式重跑仍保留原八次登记及原 TC 分类，启用模式保持实际排序规则冲突。Rust 1.99 和 1.85 各通过 633 项工作区测试、4 项文档测试和严格 Clippy。TC 存储排序规则与完整排他验收继续跟踪。 [锁表长度验证](phase-2/wp05-tc-lock-db-length-guard.json)

WP05 已从固定 Java TC 与 MySQL 8.4.13 的真实锁表确认，PAD SPACE 业务主键等价的 key 和带尾随空格的 key 可以获得两把 NO PAD 全局锁。新增可选的管理器级排序规则校验，在原事务持有元数据锁时核对文本主键列，失配在登记前回滚。启用后两组不兼容登记均未发出，匹配排序规则的对照组仍由 TC 处理冲突。Rust 1.99 和 1.85 各通过 633 项工作区测试、4 项文档测试及严格 Clippy。业务和 TC 部署一致性及完整验收仍待跟踪。 [排序规则验证](phase-2/wp05-tc-lock-collation-guard.json)

WP05 的排序规则预检改为从原事务实际解析的物理列类型取得排序规则，支持空表与忽略表名大小写的数据库。真实 MySQL 8.4.13 lower_case_table_names=1 先复现混合拼写误拒，再由 Rust 1.99 与 1.85 分别验证原键到达固定 Java TC 并正常触发锁冲突。两版本各通过 633 项工作区测试、4 项文档测试及严格 Clippy。并发改变主键成员和列类型的验收仍待完成。 [验证记录](phase-2/wp05-tc-lock-collation-table-case.json)

WP05 已复现 RR 多表事务中的旧主键字典快照，并以当前 SHOW CREATE TABLE 主键定义校验拦住过期主键。原事务被拒绝并整体回滚后，新事务读取新主键和无符号列并完成可回滚执行。Rust 1.99 和 1.85 各通过现场回归、633 项工作区测试和严格 Clippy。类型、字符集、引擎、触发器及外键独立变化继续待验收。 [跨表 DDL 验证](phase-2/wp05-metadata-cross-table-ddl.json)

WP05 已复现主键不变时的跨表列类型旧快照，新增当前建表定义逐列类型核对。原事务拒绝并污染，回滚后新事务可读取 BIGINT UNSIGNED 并准备 u64::MAX 镜像，整体回滚恢复原业务值。Rust 1.99、1.85 各通过现场回归、633 项工作区测试与严格 Clippy。类型核对只覆盖本次已验证边界，其他独立结构变化继续待验收。 [类型 DDL 验证](phase-2/wp05-metadata-type-ddl.json)

WP05 已复现旧事务将当前 MyISAM 表误认作 InnoDB 的风险，新增当前物理建表定义引擎校验。真实 MySQL 双版本用例在 DML 前拒绝非事务引擎，原事务回滚恢复表 A，表 B 未发生业务写入。包含换行和伪造引擎文本的列注释也通过拒绝验证。Rust 1.99、1.85 各通过现场回归、633 项工作区测试与严格 Clippy，自有运行环境已清理。 [引擎 DDL 验证](phase-2/wp05-metadata-engine-ddl.json)

WP05 新增物理字符集校验，已复现 UTF-8 列独立改为 ASCII 后旧字典仍被接受的问题。原事务按物理 CHARSET 与快照比较，失配后拒绝并污染，回滚后新事务正常读取 ASCII 域并完成可回滚执行。Rust 1.99、1.85 各通过现场回归、633 项工作区测试与严格 Clippy，自有环境已清理。同字符集排序规则及列顺序等边界继续待验证。 [字符集 DDL 验证](phase-2/wp05-metadata-charset-ddl.json)

WP05 新增当前物理结果列检查，独立新增列、移动列位置及改变可空性均在旧事务中被拒绝。回滚后新事务正常读取新结构，业务写入继续可回滚。Rust 1.99、1.85 各通过六项真实 MySQL DDL 回归、633 项工作区测试及严格 Clippy，自有环境已清理。新增不可见列、Extra 和触发器相关变化仍待覆盖。 [列结构 DDL 验证](phase-2/wp05-metadata-shape-ddl.json)

WP05 补齐不可见列、整数/文本生成列和自增主键的独立属性变化拒绝。物理建表列数量补足 SELECT * 的不可见列遗漏，生成属性在独立字符集声明之后仍被识别。Rust 1.99、1.85 各通过四组属性变化、七项真实 MySQL DDL 回归、633 项工作区测试及严格 Clippy，自有环境已清理。触发器、外键及通用建表输出边界继续跟踪。 [额外列属性验证](phase-2/wp05-metadata-invisible-ddl.json)

WP05 已复现跨表旧事务遗漏后提交触发器的问题，并新增同身份辅助会话查询当前触发器、入向外键与出向外键。原事务先持有目标表 MDL，辅助会话核对 server UUID 和 schema，三条公开业务入口在同一绝对期限内完成检查。三组约束变化均在业务及级联副作用之前被拒绝，整体回滚后的独立查询确认业务值和关联行状态。Rust 1.99、1.85 各通过新增现场用例、七项 DDL 回归、633 项工作区测试及严格 Clippy，183 项环境测试在普通工作区运行中忽略，自有环境已清理。每次检查增加一个临时池外连接，部署仍要求稳定单主路由。检查后外键新增的锁时序、独立 Undo 补偿读取路径及 WP05/WP06 其余验收继续待完成。 [当前约束验证](phase-2/wp05-metadata-constraint-ddl.json)

WP05 已补检查后的触发器与入向/出向外键 DDL 锁时序证据。另一会话执行三类标准 DDL 均被原事务阻塞并返回 MySQL 1205，原事务随后可执行 UPDATE、DELETE 并整体回滚，独立查询确认原值与子行保留、副作用表为空。释放后同一 DDL 成功，新事务拒绝带约束的表并保持污染。Rust 1.99、1.85 各通过新增锁时序现场用例、八项 DDL 回归、633 项工作区测试和严格 Clippy，184 项环境测试在普通工作区运行中忽略。转发进程、自有容器及隔离引擎已清理，运行脚本与摘要校验器已归档。本轮仅新增验收，独立 Undo 补偿当前约束检查及全部计划其余工作继续实施。 [检查后 DDL 验证](phase-2/wp05-metadata-postcheck-ddl.json)

WP05 已修复独立与托管 Undo 补偿的迟到约束快照缺口。真实 RED 在读取 Undo 字典后提交业务触发器，旧实现补偿成功并留下意外副作用行。修复在原控制期限和原 MDL 内，用同身份新会话核对所有业务表当前触发器及入出向外键，全部表通过后才开始补偿。真实 GREEN 覆盖迟到触发器和两类外键，拒绝后保留原分支身份、原 Undo 七列和业务/子行。Rust 1.99、1.85 各通过 16 项独立 Undo、11 项托管恢复、633 项工作区测试与严格 Clippy，普通工作区运行忽略 186 项环境测试。独立审查未发现阻断问题，自有容器、转发进程和隔离引擎已清理。辅助连接成本及稳定单主路由要求继续适用，公开一阶段提交及原计划其余门槛继续待完成。 [Undo 当前约束验证](phase-2/wp05-undo-current-constraints.json)

WP05 新增有界 PEM CA 配置，保持证书链与原主机身份校验，并与关闭回环测试 TLS 的选项双向互斥。配置阶段拒绝空解析，PEM 正文不进入 Debug。真实 MySQL 强制 TLS，正确 CA 支持连接池、元数据辅助会话及可回滚业务，缺 CA、错误 CA、错误主机名和明文连接被拒绝。独立客户端记录 TLS 1.3 与原业务值，两项有效削弱控制分别捕获信任校验绕过和主机名改写。Rust 1.99、1.85 各通过 634 项工作区测试、现场 TLS 用例及严格 Clippy，普通工作区运行忽略 187 项环境测试。最低工具链链接因 D 盘满而失败后，仅清理已过期的本任务 PDB 缓存并成功重跑，清单和失败日志保留。独立审查未发现阻断问题，自有容器、转发和隔离引擎已清理。其他证书/认证配置、公开提交与原计划其余门槛继续实施。 [TLS CA 验证](phase-2/wp05-mysql-tls-ca.json)

WP05 新增客户端证书与未加密私钥成对配置，各限制 64 KiB，保持服务端 VerifyIdentity 并与关闭 TLS 双向互斥。真实 REQUIRE X509 账号拒绝缺证书、非受信客户端、错配私钥及错误密码；正确身份经池和元数据辅助会话执行并回滚业务，独立确认原值。独立 MySQL 客户端记录同一账号、TLS 1.3 和缺身份时的 1045。Rust 1.99、1.85 各通过 635 项工作区测试、两项现场 TLS 测试及严格 Clippy，普通运行忽略 188 项环境测试。移除生产身份设置的有效削弱控制命中正向认证断言，原源码精确恢复；独立审查未发现阻断问题。全量构建前仅清理过期 PDB 缓存，自有容器、转发和隔离引擎已清理。私钥入口沿用驱动首个可识别块语义，不承诺内存副本零化；证书生命周期、其他认证配置、公开提交和全部计划其余门槛继续实施。 [X509 客户端认证验证](phase-2/wp05-mysql-mtls.json)

WP05 已补选定 RSA 证书的有效期、同 CA 叶证书及 CA 在线轮换证据，生产源码保持不变。真实 MySQL 拒绝失效客户端身份，并以 3888 拒绝失效服务端重载后保留旧上下文；客户端面对失效服务端证书的握手拒绝由双 TLS 前端连接真实 MySQL 验证，未混称为 MySQL 直接呈现坏证书。换 CA 后旧信任的新池及辅助握手被拒绝，原事务污染、回滚且独立原值为 10；同一个双 CA 数据源在两端新握手均成功。Rust 1.99、1.85 各通过四线程运行下的四项现场测试、635 项工作区测试及严格 Clippy，普通运行忽略 192 项环境测试。测试内加入进程锁并补齐双 CA 旧端断言，独立复核未发现阻断问题。有效削弱控制命中过期证书断言，源码精确恢复；磁盘链接失败及过期 PDB 清理清单保留。自有 MySQL、双 TLS 前端、中继和隔离引擎已清理。原 WP05/WP06、生产接入及其余完整计划门槛继续实施。 [TLS 生命周期与轮换验证](phase-2/wp05-mysql-tls-lifecycle.json)

WP05 已修复 Undo 实际引擎变化被旧字典快照掩盖的原子性缺口。真实 RED 在业务元数据读取后将 undo_log 改为 MyISAM，旧 flush 返回成功，业务回滚后独立观察业务 0 行、非事务 Undo 1 行。新校验在原 MDL 和绝对期限内核对当前物理引擎，写入前拒绝并保持事务污染。三组现场案例包含带尾空格完整伪引擎词元的列/表注释，原始 SHOW CREATE 字节确认当前配置只有真实 MyISAM 引擎行；恢复 InnoDB 后新事务可 flush 并整体回滚。Rust 1.99、1.85 各通过 11 项写入、16 项补偿和 1 项业务引擎现场回归、635 项工作区测试与严格 Clippy，普通运行忽略 193 项环境测试。共享夹具重叠造成的无效日志已保留并经顺序重跑替代，审查指出的注释负例及断言位置均已修正复核，无新增阻断发现。自有数据库、中继和隔离引擎已清理。其他受保护表的元数据快照与原 WP05/WP06 门槛继续实施。 [Undo 当前引擎验证](phase-2/wp05-undo-engine-ddl.json)

WP05 已将既有 Undo 规范在同身份新元数据事务中复核，修复旧快照漏过迟到触发器的非事务副作用。真实 RED 在业务/Undo 回滚后仍留下副作用 1 行；新校验覆盖触发器、唯一索引删除、列宽、不可见列、无符号列及入出向外键七组，拒绝后保持污染并回滚，各表无残留。原事务持有 MDL，新辅助事务核对当前规范后回滚并关闭；Normal、围栏及授权清理三个中央入口均接入原认证/TLS 配置，原错误优先级和绝对期限保持。Rust 1.99、1.85 各通过 12 项写入、16 项补偿、9 项围栏、1 项受控原包提交清理回归、635 项工作区测试与严格 Clippy，普通运行忽略 194 项环境测试。独立审查未发现阻断，确认无递归建连接或现有生产绕过路径。每次校验增加一个池外连接，并发 DDL 可导致按原期限安全失败；稳定单主路由要求保持。自有数据库、中继和隔离引擎已清理。恢复表快照及原全部计划剩余门槛继续实施。 [当前 Undo 规范验证](phase-2/wp05-undo-current-schema.json)

WP05 已修复恢复表旧字典快照遗漏实际定义变化导致的伪提交见证。真实 RED 将恢复表改为 MyISAM 后，旧仲裁仍成功，进程退出后的独立查询确认业务 0、Undo 0、first_phase_committed 1。新校验在原 MDL 下，以原认证/TLS 的同实例/schema 辅助事务重跑恢复表既有规范，并拒绝新旧能力版本不一致；提交前置、恢复控制和独立见证观察三个入口统一接入。七类迟到 DDL 与合法 003→004 迁移均安全拒绝，保留真实分支回执，业务/Undo/见证/触发器副作用均为零；允许 MySQL 1412 的用例不单独宣称 Rust 版本差异分支覆盖。Rust 1.99、1.85 各通过 13 项提交前置、11 项准备、11 项恢复、1 项受控清理、635 项工作区测试和严格 Clippy，普通运行忽略 196 项环境测试。MSRV 首次准备回归因前次自有辅助库残留失败，原日志保留，精确重置夹具后重跑通过；断言未削弱。独立只读审查无阻断项，证据校验通过，自有数据库、中继与隔离引擎已清理。每次校验增加一个池外连接，稳定单主及原期限边界保持；公开业务 COMMIT、原 TC 安全和 WP05/WP06 完整计划门槛继续实施。 [恢复表当前规范验证](phase-2/wp05-recovery-current-schema.json)

WP05 已修复可选资源绑定表的旧字典引擎快照缺口。真实 RED 在业务事务读取元数据后把绑定表改为 MyISAM；旧准备仍发送 BranchRegister，且准备作用域未结束时管理员成功改写绑定行，证明所需共享锁失效。新实现在原 FOR SHARE 的 MDL 下复核当前物理引擎，迟到 MyISAM 和视图均在登记前拒绝；回执/登记未知均不产生，业务、Undo、恢复记录零。Rust 1.99、1.85 各通过 12 项准备、13 项提交前置现场回归、635 项工作区测试与严格 Clippy，普通运行忽略 197 项环境测试。独立只读审查无阻断，证据校验通过，自有数据库、中继及隔离引擎已清理。首次链接因磁盘压力和 PDB 错误失败，清理限定的过期/中断 PDB 后重建通过，原诊断保留。完整绑定表规范、原 TC 排他安全、公开业务 COMMIT、WP05/WP06及完整计划继续实施。 [资源绑定当前引擎验证](phase-2/wp05-binding-current-engine.json)

WP05 对固定原版 Java TC MySQL DB 锁存储新增显式组合配置 with_java_tc_mysql_db_lock_profile，一次启用既有资源绑定、锁表字段长度和文本主键排序规则校验，保留原单项配置与 TLS。TDD 编译 RED 是方法缺失；Rust 1.99/1.85 各通过 2 项定向测试、严格 workspace Clippy 和格式检查。此入口只减少配置遗漏，部署仍需核对原 TC 实际表、005 绑定、同一物理主库及所有写入器原 JDBC ID；完整锁安全和公开业务 COMMIT 仍开放。[组合配置证据](phase-2/evidence/java-tc-mysql-db-profile/)

WP05 固定原 Java TC MySQL DB 锁存储组合配置已完成选定场景现场验证。首次复用历史 as_cs 文本主键夹具被组合排序规则校验安全拒绝，失败原件留存；组合专用夹具使用与原 TC 锁表相同的 ai_ci 后，Rust 1.99/1.85 各运行六组物理资源别名/复合键身份案例，原 RM/TM 帧分别核对 3 次登记（2 成功、1 冲突）、2 次锁查询、6 次全局回滚和 3 次本地登记前拒绝。原 TC 回调分别 3/4 次、重试 1/2 次；独立最终数据库均无全局事务、分支、锁和业务行。当前源码重跑后两工具链严格 Clippy、格式、源码/JAR/驱动/编译目标绑定与自有运行环境清理通过。场景范围外的完整排他覆盖、外部客户端及公开业务 COMMIT 仍 OPEN。[组合原 Java TC 联试](phase-2/wp05-java-tc-mysql-db-profile.json)
# 2026-11-24：WP05 一阶段见证能力门槛

`MySqlCommitCheckedTransaction::require_first_phase_commit_witness` 现在只在原事务内成功暂存 004 见证时通过。旧 003 表继续支持原有可回滚仲裁，但不会被误用为有见证提交资格；无变化分支也不伪称已暂存见证。测试先得到缺少该方法的 E0599，再在自有 MySQL 8.4.13 分别以 003/004 实际表结构执行成功；Rust 1.99/1.85 各通过 14 项提交前仲裁现场回归、完整 workspace 严格 Clippy，格式检查通过。自有中继、容器和隔离引擎已按 PID、容器 ID 与标签核对后关闭。此项只建立后续 COMMIT 的必要条件；COMMIT、未知结果恢复、完整 TC 排他覆盖及原 WP05/WP06 仍未完成。

# 2026-11-24：WP05 无变化本地提交

在已仲裁作用域增加消费式 `commit_no_change`：仅无 Undo、无登记回执、无候选 004 见证时发送一次原连接 COMMIT；已登记分支在发送前拒绝并保留原 BranchId。驱动调用失败保守分类为未知，不重放 SQL。测试先得到缺少方法的 E0599；自有 MySQL `general_log` 证明实际 COMMIT 一次，原连接被断开后返回未知，已登记分支拒绝且业务与 Undo 回滚。Rust 1.99/1.85 各通过 17 项提交前仲裁现场回归、完整 workspace 严格 Clippy，格式检查通过。有变化业务 COMMIT、服务器已执行但回包丢失、报告及旧尝试围栏仍开放。

# 2026-11-24：WP05 无变化 COMMIT 确认丢失

自有 MySQL 故障代理逐帧转发并截留原 COMMIT 的服务端 OK。旧实现真实 RED 为 1.5 秒看门狗到期，新增原操作期限后 Rust 1.99/1.85 均在 350 毫秒预算内返回未知；代理标记证明服务端已给出 OK，独立 `general_log` 增量为 1。两工具链各复跑 4 项无变化/分支拒绝/断连现场回归，严格全工作区 Clippy 和格式检查通过。[现场证据](phase-2/evidence/mysql-no-change-commit-loss/README.md)。这不关闭有变化业务 COMMIT、TC 报告、旧尝试围栏或 WP05/WP06 原验收。

# 2026-11-24：WP05 受影响行必须有 Undo 镜像

`execute` 现在拒绝“受影响行数为正但前后镜像全空”，保持事务中毒，防止把异常业务写入误归入无变化提交。先得到 E0599，双 Rust 工具链各通过单元反例、17 项真实 DML 正向回归和一次无变化 COMMIT；严格 Clippy 与格式检查通过。[现场证据](phase-2/evidence/mysql-affected-image-guard/README.md)。尚未注入真实驱动错误镜像形状，公开有变化 COMMIT 和原 WP05/WP06 仍未完成。

# 2026-11-24：WP05 一阶段见证联合 Undo 观察

独立观察接口现在对 `first_phase_committed=1` 且二阶段未终结的原分支锁定并读取 Normal Undo；原 Undo 缺失返回恢复冲突，已终结且合法删除 Undo 时继续保留历史见证。真实 MySQL 故意删 Undo 的旧实现 RED、修复后 GREEN 已留存；Rust 1.99/1.85 各通过 17 项提交前仲裁现场回归和严格工作区 Clippy，格式检查通过。[现场证据](phase-2/evidence/mysql-witness-undo-observation/README.md)。业务行逐一核对、公开有变化 COMMIT、原 TC 排他锁完整覆盖及原 WP05/WP06 仍开放。

# 2026-11-24：WP05 一阶段见证的 Undo 正文与身份

观察端在二阶段未终结时进一步解码原 Jackson Undo，核对 XID、BranchId 和非空类型日志。自有 MySQL 上改写 `rollback_info` 为无效 JSON 得到旧实现 RED；修复后无效正文和错误分支号均被拒绝，原正文和二阶段合法清理继续通过。Rust 1.99/1.85 各完成 17 项现场回归、严格工作区 Clippy 与格式检查。[现场证据](phase-2/evidence/mysql-witness-content-observation/README.md)。物理表与业务行重验、公开有变化 COMMIT、原 TC 排他锁完整覆盖及 WP05/WP06 继续开放。

# 2026-11-24：WP05 有变化事务本地 COMMIT 实验

默认关闭的 `experimental_changed_commit` 特性现可消费已仲裁作用域，原连接一次 COMMIT 业务、Normal Undo 和 004 见证；成功还要求独立见证确认。发送前拒绝与提交/观察未知分开保留真实回执，不向 TC 发送一阶段报告。自有 MySQL 的 E0599 编译 RED 后，两 Rust 工具链各通过 20 项现场仲裁回归和新增的提交后观察者故障单例；一次 COMMIT 日志计数、旧表拒绝、断连未知及独立观察恢复均通过。默认和实验特性两种构建各通过严格 Clippy，格式检查通过。[实验现场证据](phase-2/evidence/mysql-changed-commit-lab/README.md)。原 Java TC 完整排他锁、旧尝试围栏、正式报告和生产公开提交仍未完成；WP05/WP06 原门禁不变。

## 2026-11-24：WP05 实验提交的 MySQL COMMIT OK 响应丢失

在默认关闭的 `experimental_changed_commit` 中，以帧代理转发原 `COMMIT`，确认 MySQL 服务器实际返回 OK 后扣留响应。Rust 1.99 和 1.85 的故障单例均返回携原登记回执的 `Unknown`，不重复发 COMMIT；独立连接看到业务行、Normal Undo 和 004 见证持久化，`mysql.general_log` 各仅增加一次 COMMIT。两工具链各自另有 21 项现场仲裁回归通过，默认与实验特性严格 Clippy、格式检查通过。[现场证据](phase-2/evidence/mysql-changed-commit-ok-loss/README.md)。本测试使用受控原协议对端；原 Java TC 排他锁、旧尝试围栏、正式一阶段报告及完整 WP05/WP06 门禁仍开放。

## 2026-11-24：WP05 实验一阶段成功报告与响应丢失

默认关闭的实验特性新增 `commit_changed_and_report`：只有原本地 COMMIT 和独立持久见证确认后才发送一次原 `PhaseOne_Done`。受控原报文对端在报告到达时独立核对业务、Normal Undo、004 见证；双 Rust 工具链均通过报告成功、收到报告后断连的未知分类、原连接失败时禁止成功报告。各自 23 项现场回归、默认和实验特性严格 Clippy 及格式检查通过。[实验报告证据](phase-2/evidence/mysql-changed-commit-report/README.md)。原 Java TC 锁覆盖、旧尝试持久围栏、明确失败报告和完整 WP05/WP06 仍开放。上一轮误查 Alpine WSL 导致运行时清理状态写错；本轮在正确的 Ubuntu-20.04 中复核并清理了原自有容器和隔离引擎，原证据已更正。

## 2026-11-24：固定 Java TC 有变化提交／报告联试待运行

新增正式 Rust TM/RM 与原 Java TC 的实验特性现场用例及固定 JAR/认证/原帧 runner。Rust 1.99 和 1.85 的目标测试编译、严格 Clippy 及格式检查通过。现场容器初始化时 D 盘空间不足、Docker 出现 `SIGBUS`/I/O 错误；清理本项目可重建构建产物后，WSL 仍以 `HCS_E_CONNECTION_TIMEOUT` 拒绝启动。原 Java TC 用例未执行；额外完整包测试也因磁盘不足失败，均不计为通过。[尝试证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。正式一阶段报告、排他锁与 WP05/WP06 门禁继续开放。

## 2026-11-24：WP05 明确回滚后失败报告入口

默认关闭的实验特性新增消费式 `rollback_and_report_failure`：原本地回滚确认且持真实登记回执后，才单次报告 `PhaseOne_Failed`；回滚与报告错误分层保留原回执，报告响应丢失保留回滚事实。三个真实 MySQL 用例已写入，分别检查报告前业务/Undo 不可见、报告丢响应及回滚前断连不报告；Rust 1.85/1.99 的特性测试目标实际链接、严格 Clippy 与格式检查通过。WSL 持续返回 `HCS_E_CONNECTION_TIMEOUT`，用例及固定原 Java TC 联试均**未运行**，不计 WP05 验收。原排他锁安全、旧尝试围栏、完整故障矩阵及 WP05/WP06 门禁继续开放。

## 2026-11-24：WP05 SQL 识别变异边界回归

从三类已支持 DML 各生成512个固定种子局部变体，检查解析不panic，并核对任何成功结果仍保留原 SQL、参数上限与DML类型。Rust 1.99/1.85 各运行19项 SQL 识别测试通过，目标严格 Clippy 与格式检查通过。覆盖的是有界输入安全性；真实 MySQL 行镜像、Undo、失败报告现场及原 Java TC 联试仍未验收。

## 2026-11-24：一阶段失败报告的 RM 原 TCP 拒绝分类

受控原 TCP 对端实际收到单次 `PhaseOne_Failed`，返回 `BranchReportFailed`。正式 RM 客户端将其归为明确拒绝，保留完整原请求/响应，调试输出不泄漏对端错误文本，也不自动重发。Rust 1.85/1.99 各运行6项 `native_calls` 测试通过，严格 Clippy 与格式检查通过。该控制面验证不证明 MySQL 已回滚，固定原 Java TC 与 WP05 现场门禁仍未完成。

## 2026-11-24：WP05 注册后准备失败窗口核对

源码核对发现 `MySqlTransactionPreparer::prepare` 在取得真实分支回执后，`enroll` 或 Undo 写入失败会返回携回执的错误，但原业务事务由作用域丢弃，没有显式 ROLLBACK 成功确认。现有实验 `rollback_and_report_failure` 只接受已经返回的准备作用域，不能覆盖这两个失败窗口。已在[一阶段提交契约](phase-2/mysql-first-phase-commit-contract.md)补充显式回滚、单次失败报告、取消/期限保守分类及固定原 Java TC 故障注入要求；代码与现场验收仍未完成，不计 WP05 原任务。

## 2026-11-24：WP05 注册后准备错误显式回滚

`MySqlTransactionPreparer` 在已取得真实回执后遇到明确 `enroll` 或 Undo 错误时，现以原入口剩余期限显式回滚原业务事务，并在原错误中保存回滚结果；回滚未获确认时 `local_rollback_confirmed` 保持 false。现有两条真实 MySQL 失败用例增加成功回滚断言，另新增原连接断开后保留回执但不得标记回滚确认的负向用例。Rust 1.85/1.99 的全测试目标严格 Clippy 通过；WSL 仍在创建实例时超时，三个真实用例未运行。准备错误的自动单次失败报告、固定原 Java TC 联试及完整 WP05/WP06 仍开放。

## 2026-11-24：WP05 准备错误的单次失败报告入口

默认关闭的实验特性新增 `MySqlPrepareError::report_failure_after_rollback`：仅在显式回滚确认及真实分支回执存在时报告原 `PhaseOne_Failed`。错误对象保留回执，原子门闩在报告调用开始时占位；响应丢失、等待取消或再次调用均不重发。受控原 TCP 测试实际验证了原身份字段、一次成功报告、取消后回执仍可查询、再次调用拒绝和回滚未确认不发送；Rust 1.85 两项报告测试及32路门闩竞争测试通过，1.85/1.99 的默认与实验特性严格 Clippy 通过。准备错误后的自动调用、真实 MySQL 结果及固定原 Java TC 联试仍未验收。

## 2026-11-24：WP05 准备失败报告的 TC 拒绝分类

新增受控原 TCP 测试：TC 对原 `PhaseOne_Failed` 返回 `BranchReportFailed` 时，准备错误保留原响应并分类为明确拒绝，不暴露服务端私有消息，单次门闩阻止第二次报告。Rust 1.99 的该模块 3 项测试及完整库测试通过（43 通过、4 项需外部 MySQL 而忽略），默认与实验特性严格 Clippy、格式检查通过。本轮释放本项目专用 Rust 1.85 构建缓存约 3.6 GiB 后，Ubuntu WSL 仍返回 `HCS_E_CONNECTION_TIMEOUT`；原 Java TC/MySQL 现场及新增测试的 Rust 1.85 复跑未完成，WP05/WP06 门禁仍开放。

## 2026-11-24：WP05 准备错误实验报告编排

新增特性门控的 `prepare_transaction_and_report_failure`：沿用原准备操作，只有已登记分支在原连接显式回滚确认后才自动尝试单次 `PhaseOne_Failed`，分层返回原准备错误与报告结果。原接口保持兼容。新增受控 TCP 测试先因编排类型缺失而失败，实施后验证已确认错误触发报告、未确认回滚不报告，原身份和错误仍可查询；Rust 1.99/1.85 各四项报告目标测试与完整库测试通过（各 44 通过、4 项需外部 MySQL 而忽略），默认与实验特性严格 Clippy、格式检查通过。真实 MySQL 失败路径和固定原 Java TC 尚未验收，不能关闭 WP05 门禁。

## 2026-11-24：WP05 报告等待取消的原错误保全

审查上轮编排入口发现：若调用 Future 在失败报告等待中被取消，原错误仅由该 Future 持有，连同真实回执会一起丢失。现要求调用方提供初始为空的错误槽，在任何报告网络等待前保存原准备错误；完成后将错误及独立报告结果归还，取消后槽中保留原回执和已尝试门闩。受控 TCP 测试先因接口缺少错误槽而编译失败，再验证取消后的回执仍可读取且重复报告被拒绝。准备阶段本身被取消时的保全仍开放；该入口仅在实验特性下，真实 MySQL 与原 Java TC 现场仍未验收。

## 2026-11-24：WP05 准备阶段取消的原登记进度保全

实验编排入口现于 `BranchRegister` 等待前在调用方错误槽记录“申请可能生效、回执未知”，收到真实回执后更新原 `BranchId`；后续 `enroll`、Undo 或显式回滚等待被取消时，调用方仍持原身份但不误报回滚确认。新增受控原 TCP 取消测试先因阶段记录方法缺失而编译失败，再验证两个阶段在 Future 取消后分别保留未知分类与真实回执，且没有本地回滚确认时禁止 `PhaseOne_Failed`。Rust 1.99/1.85 各 45 项库测试通过、4 项需外部 MySQL 而忽略；实验特性严格 Clippy 通过。该测试只覆盖阶段记录和受控协议，不证明真实 MySQL 准备 Future 在每个故障点的完整行为；固定原 Java TC/MySQL 现场仍未运行。

## 2026-11-24：WP05 真实 MySQL 编排现场用例待运行

在现有自有库夹具加入两条用例：实验特性下 Undo 唯一键冲突后单次失败报告，并在报告到达时独立检查业务/Undo/控制表事实；默认构建下注册请求已发送后取消准备 Future，检查调用方错误槽的未知登记状态。Rust 1.99/1.85 的两个目标测试编译链接和严格 Clippy 均通过。当前本机无原生 `mysqld`/MySQL 服务，WSL/Docker 运行时仍不可用，因此两条用例**未运行**，尚无真实数据库或原 Java TC 验收证据。

## 2026-11-24：WP05 默认准备接口的取消进度

默认构建新增 `prepare_transaction_with_progress`，将注册请求可能已生效及真实回执两阶段保存在调用方空错误槽；正常返回清除中间态并返回原准备结果。实验失败报告入口复用该方法，原 `prepare_transaction` 保持兼容但没有外部取消保全。现有自有 MySQL 取消用例改为覆盖默认接口，先因方法缺失而编译失败；实施后 Rust 1.99/1.85 的默认测试目标编译链接、严格 Clippy 通过。真实 MySQL 运行及固定原 Java TC 验证仍未完成。

## 2026-11-24：WP05 自有 Windows MySQL 现场恢复

从 Oracle 官方取得与既有固定镜像同为 8.4.13 的 Windows ZIP，MD5 与官方公布值一致；仅在 D 盘建立隔离数据目录和回环端口。`SELECT VERSION()`、InnoDB、`REPEATABLE-READ` 与实际数据目录已记录。Rust 1.99/1.85 分别实际通过准备目标 15 项、提交前置条件目标 26 项；独立武装 MySQL 帧代理截留原 COMMIT OK 后，两版本各通过 1 项有变化提交未知结果用例。此前未运行的三条回滚后失败报告用例均包含在 26 项中。首次整组准备运行因自有 `_other` 库已存在失败 1 项，夹具改为幂等建库后双版本完整复跑通过。原始日志、MySQL 事实及 OK 标记见[现场证据](phase-2/evidence/mysql-native-prepare-20261124/README.md)。本轮仍未执行固定原 Java TC，外部锁风险、旧尝试围栏、跨进程恢复和 WP05/WP06 生产门禁继续开放。

## 2026-11-24：WP05 固定原 Java TC 有变化提交与成功报告

在原 runner 加入 D 盘自有 Windows MySQL 后端，运行前核对 8.4.13、回环端口与独立数据目录，并沿用原 Java pin、JAR 与 Connector/J 哈希。首次现场运行原 TC 拒绝 RM 注册，原帧与日志定位为认证夹具未收到本次精确资源名；补入启动参数后，Rust 1.99/1.85 各通过选定的有变化 COMMIT／单次 `PhaseOne_Done`／GlobalCommit／原回调清理测试。两次独立数据库最终的 `global/branch/lock/business/undo/first_phase_committed/commit_terminal` 计数均为 `0/0/0/1/0/1/1`，原始帧及进程清理记录见[原 TC 现场证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。此为 Windows MySQL 与认证测试覆盖层的选定成功路径，未证明全部外部写入器排他锁、旧尝试持久围栏、失败与未知报告或 WP05/WP06 生产 Gate。

## 2026-11-24：WP05 固定原 Java TC 明确回滚与失败报告

新增独立数据库现场用例：原 Java TC 已真实登记并持锁时，Rust 先确认业务/Normal Undo 对其他连接不可见，再在原连接显式回滚并单次报告 `PhaseOne_Failed`；固定原 TC 返回同请求 ID 的报告成功响应，TM 随后 GlobalRollback。Rust 1.99/1.85 各经 runner 通过 1/1，终态全局、分支、锁、业务、Undo 与提交见证均为零；成功提交／报告用例也在强化后的同一 runner 下双版复跑通过。两工具链目标严格 Clippy、格式检查通过，原始帧、数据库事实和进程退出记录见[联试证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。准备内部错误后的失败报告、响应丢失、旧尝试围栏、外部锁覆盖及完整 WP05/WP06 Gate 继续开放。

## 2026-11-24：WP05 固定原 Java TC Undo 错误后的自动报告

在自有数据库原 Undo 表创建仅供测试的拒绝触发器，使固定原 Java TC 的真实 BranchRegister 成功后，原连接的 Normal Undo 写入明确失败。`prepare_transaction_and_report_failure` 保留原 BranchId 与 `Undo` 错误，显式回滚成功才自动发送一次 `PhaseOne_Failed`。Rust 1.99/1.85 各通过 1/1；原帧显示单次注册、单次失败报告及同请求 ID 成功响应，GlobalRollback 后全局、分支、锁、业务、Undo 与提交见证七项计数全为零。最初测试夹具用预处理协议执行触发器 DDL 被 MySQL 拒绝，改用原始 DDL 后双版复跑通过；两工具链目标严格 Clippy 与格式检查通过。见[原 TC 证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。`enroll` 错误、回滚/报告丢响应、旧尝试围栏、外部锁覆盖及完整 WP05/WP06 Gate 继续开放。

## 2026-11-24：WP05 原 TC 准备错误证据校正与控制表安全拒绝

进一步检查实际源码发现，原 Undo 表的测试触发器在 INSERT 前被元数据安全检查拒绝；上节“Normal Undo 写入明确失败”应按此更正为“Undo 阶段安全拒绝”，不能作为真实 INSERT 错误的证据。新增控制表触发器场景，同样在写入前触发 `Enrollment` 安全拒绝；Rust 1.99/1.85 各通过 1/1，原回执、确认回滚、单次失败报告及成功响应均保留。独立核对：控制表拒绝时控制行 0，Undo 表拒绝时控制行 1，两者 GlobalRollback 后七项计数全零。两种用例共享的源码已改名并增加 Undo 错误来源精确断言；磁盘容量短暂耗尽后，使用 D 盘构建目标并在空间恢复时于固定原 Java TC 双版复跑改名后的 Undo 用例，各通过 1/1。真实 Undo 唯一键冲突目前只在受控协议对端的自有 MySQL 通过，仍需固定原 TC 联试。见[原 TC 现场证据](phase-2/evidence/java-tc-changed-commit-report/README.md)；回滚/报告丢响应、旧尝试围栏、外部锁覆盖及完整 WP05/WP06 Gate 继续开放。

## 2026-11-24：WP05 固定原 Java TC 的真实 Undo 唯一键冲突

新增原 TC 现场用例：测试连接暂锁自有控制表，使原 TC 先完成单次 BranchRegister；从原 `branch_table` 读取真实 BranchId，再由独立连接为同一 XID/BranchId 预置 defense Undo，释放锁后让 Rust 准备继续。原事务在 `Undo insert` 实际收到 SQLSTATE `23000` / MySQL 1062，错误保留原回执；显式回滚确认后自动单次 `PhaseOne_Failed`，原 TC 同请求 ID 成功响应，TM GlobalRollback。Rust 1.99/1.85 各通过 1/1，最终全局、分支、锁、业务、提交见证均为零，只有预置冲突 Undo 留存。两工具链目标严格 Clippy、格式检查与原始帧/进程清理核对通过；见[现场证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。该用例补齐真实 Undo INSERT 错误的选定原 TC 路径，控制登记真实写入故障、回滚/报告响应丢失、旧尝试围栏、外部锁覆盖及完整 WP05/WP06 Gate 仍开放。

## 2026-11-24：WP05 固定原 Java TC 失败报告成功回包丢失

原帧代理新增只针对 RM `BranchReportResponse` 的故障模式：先确认原 Java TC 对同一请求 ID 返回成功字节，再单次截留并关闭连接，禁止第二次报告。已准备分支先在原连接明确回滚，报告 Future 因原回包丢失返回未知；Rust 保留原 BranchId 与本地回滚事实，不自动重发。Rust 1.99/1.85 各通过 1/1，原 TC 随后 GlobalRollback，七项终态计数全零；普通透传失败报告路径也在 Rust 1.99 复跑通过。原始帧、交付数、数据库事实与进程清理见[原 TC 证据](phase-2/evidence/java-tc-changed-commit-report/README.md)；双工具链目标严格 Clippy 与格式检查通过。准备错误和成功报告的回包丢失、回滚丢确认、旧尝试围栏、外部锁覆盖及完整 WP05/WP06 Gate 仍开放。

## 2026-11-24：WP05 固定原 Java TC 成功报告回包丢失

已准备有变化分支的本地 COMMIT 独立确认后，Rust 对固定原 Java TC 单次报告 `PhaseOne_Done`。原 TC 返回同请求 ID 成功字节，帧代理截留并关闭连接；Rust 保留真实 BranchId 与本地提交事实，报告结果标记未知，不自动重发。Rust 1.99/1.85 各通过 1/1，原 TC 后续 GlobalCommit 和回调完成，七项终态计数各为 `0/0/0/1/0/1/1`；Rust 1.99 普通透传成功路径复跑通过。原始帧、数据库事实及进程清理见[原 TC 证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。准备错误报告丢响应、回滚丢确认、跨进程恢复、旧尝试围栏、外部锁覆盖和完整 WP05/WP06 Gate 仍开放。

## 2026-11-24：WP05 准备错误自动失败报告回包丢失

固定原 Java TC 的 Undo 表安全拒绝造成准备错误；原连接显式回滚确认后，自动发送一次 `PhaseOne_Failed`。原 TC 同请求 ID 的成功响应被帧代理截留；Rust 保留原准备错误、真实分支回执和本地回滚确认，报告结果为未知，门闩拒绝重发。Rust 1.99/1.85 各通过 1/1，GlobalRollback 后七项终态计数全零；Rust 1.99 正常透传用例复跑通过。见[原 TC 证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。此用例在真实 Undo INSERT 前拒绝；真实控制登记写入故障及其报告丢失、跨进程恢复、旧尝试围栏、外部锁覆盖与完整 WP05/WP06 Gate 仍开放。

## 2026-11-24：WP05 固定原 Java TC 控制登记真实写入冲突

从原 TC 的 `branch_table` 读取真实 BranchId，给同一 XID/BranchId 预置回调归属控制行；Rust 的原控制登记 INSERT 在 MySQL Performance Schema 中实际得到 1062／23000。恢复层将重复身份归类为 `CallbackObserved` 并保护既有行；准备层保留原回执，本地回滚确认后单次 `PhaseOne_Failed`，原 TC 随后 GlobalRollback。测试预置行在报告前被独立核对，随后明确清除；七项终态计数全零。Rust 1.99/1.85 各通过 1/1，Rust 1.99 原控制表安全拒绝路径复跑通过。第一次运行客户端通过、独立事件断言因 SQL_TEXT 为空失败；修正二进制预处理事件过滤后完整重跑。原始错误事件、帧和进程记录见[固定原 TC 证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。其他控制写入错误、报告回包丢失、跨进程恢复、旧尝试围栏、外部锁覆盖及完整 WP05/WP06 Gate 仍开放。

## 2026-11-24：WP05 控制登记真实冲突后的报告回包丢失

在原 TC 分配的真实 XID/BranchId 上再次制造控制登记 INSERT 的 1062／23000，自动失败报告的原成功回包由代理截留。Rust 保留 `CallbackObserved` 准备错误、原回执和已确认本地回滚，将报告结果归类未知，拒绝重发。Rust 1.99/1.85 各通过 1/1，原 TC GlobalRollback 后七项计数全零；Rust 1.99 正常回包路径复跑通过。MySQL 独立错误事件、原 RM/TM 帧和进程记录见[固定原 TC 证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。其他写入错误、跨进程恢复、旧尝试围栏、外部锁覆盖与完整 WP05/WP06 Gate 仍开放。

## 2026-11-24：WP05 旧提交尝试恢复设计核对

源码核对确认，实验性 `commit_changed` 的 Unknown 回执仅随 Future 返回；取消或进程退出没有独立持久尝试记录。RM 的 `FileBranchStore` 保存二阶段回调身份与状态，不能还原原 BranchRegister 成功响应或锁键。已有 `fence_undo_log` 能在原 Undo 唯一键写入持久防悬挂行，但还未与提交见证及原尝试记录组成跨进程 `NotApplied` 判定。已在[一阶段提交契约](phase-2/mysql-first-phase-commit-contract.md)补充尝试记录、持久围栏、重读见证和双工具链子进程故障门禁；未据此宣称实现完成。本轮 D/E 盘剩余空间不足以运行新的链接测试，针对本任务 D 盘生成的 PDB 调试文件的精确清理被自动审批策略拒绝，文件未删除。下一步仍需实现并现场验证该恢复路径；外部锁安全与完整 WP05/WP06 Gate 不变。

## 2026-11-24：WP05 原提交尝试 Armed 持久交接

默认关闭的有变化提交实验入口现在要求 006 InnoDB 提交尝试表。原回执身份、锁键、应用数据、JDBC 配置与响应码先在独立控制事务保存为 `Armed`，成功确认后才允许原业务连接发送 COMMIT；同键不同身份、缺表、不安全表结构均明确拒绝，显式回滚原业务事务。若控制 COMMIT 结果未知，保留原回执并禁止业务 COMMIT。真实 MySQL 上 Rust 1.99/1.85 各有 29/29 提交前置条件用例与 1/1 服务器已回 OK、原业务 COMMIT 响应丢失用例通过；固定原 Java TC 的双工具链成功提交及成功报告回包丢失四次运行也独立核对 006 行。见[本地证据](phase-2/evidence/mysql-commit-attempt-arm/README.md)和[原 TC 证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。尚未实现新进程加载原尝试并结合 Undo guard/004 见证给出 `NotAppliedFenced`，也未覆盖外部写入器的锁安全；WP05/WP06 完整门禁仍开放。
## 2026-11-24：WP05 原提交尝试的首段跨进程恢复

默认关闭实验特性新增 `MySqlCommitAttempt` 持久身份读取与 `recover_commit_attempt`：按原 006 行核对资源、锁键、应用数据、成功响应码和 JDBC 配置；不签发新的 `RegisteredBranch`。恢复先独立核对 004 见证；未见提交时以原 Undo 唯一键持久围栏，再重读见证，只有 guard 已确认且见证仍为零才返回 `NotAppliedFenced`。缺记录、冲突、期限、围栏或查询故障均按 Unknown 错误处理，不重报 TC、不重放业务。真实 MySQL 上 Rust 1.99/1.85 各 33/33 项提交前置条件用例通过；两工具链均覆盖旧事务回滚前等待、旧事务提交后 `Applied`、子进程在 Armed 后原 COMMIT 前退出并由独立进程恢复。服务端已确认原业务 COMMIT、客户端丢失 OK 时，恢复接口也在双工具链单例中返回 `Applied`。原始日志见[恢复证据](phase-2/evidence/mysql-commit-attempt-arm/README.md)。仍未覆盖 COMMIT 已发送后的进程退出、围栏回包丢失、恢复查询故障、固定原 Java TC 完整恢复联试及外部锁覆盖；WP05/WP06 Gate 保持开放。
## 2026-11-24：WP05 恢复围栏丢包与 COMMIT 双侧进程故障

在上一段 Armed 跨进程恢复上补齐原围栏控制 COMMIT 成功回包丢失、恢复查询连接失效，以及原业务 COMMIT 包已发出但服务器未收到／服务器已提交而回包被扣留时终止写入子进程。恢复者均从原 006 身份重新读取：控制围栏回包丢失或查询失败时首次只返回 Unknown；独立确认 guard 后的显式再次恢复才返回 `NotAppliedFenced`。原业务未送达时新进程读到业务=0、guard=1、见证=0；服务器已提交时读到业务=1、Normal Undo=1、见证=1、guard=0 并返回 `Applied`。Rust 1.99/1.85 各 35/35 项普通真实 MySQL 回归通过，四项代理用例分别执行，原始日志见[故障矩阵证据](phase-2/evidence/mysql-commit-attempt-arm/README.md)。仍缺固定原 Java TC 在这些进程故障下的一次性原帧、全局终态和回调核验，以及外部写入器锁覆盖；完整 WP05/WP06 Gate 保持开放。
## 2026-11-24：WP05 缺失 004 行的否定判定修复

恢复故障审查发现 006 已有 Armed 但原 004 恢复行被移除时，旧实现把缺行视作见证零，实际可能错误返回 `NotAppliedFenced`。真实 MySQL 红灯已复现。恢复入口现强制要求原 004 行存在；缺行返回 Unknown 且不写 guard，原普通回执观察 API 的缺行语义不变。Rust 1.99/1.85 该负向用例及各 36/36 项普通现场回归通过；证据见[恢复矩阵](phase-2/evidence/mysql-commit-attempt-arm/README.md)。此修复只收紧本地否定判定，固定原 Java TC 完整跨进程联试和外部锁覆盖仍未完成。

## 2026-11-24：WP05 原 TC 已提交恢复后报告联试

在固定原 Java TC 和自有 MySQL 上，新增原业务提交后先按 006/004 读取 `Applied`、再用原回执单次报告 `PhaseOne_Done` 的场景。Rust 1.99/1.85 各通过 1/1；runner 独立核对原注册、报告、GlobalCommit、TC 回调、006 身份与数据库终态，原始证据见[固定原 TC 运行](phase-2/evidence/java-tc-changed-commit-report/README.md)。当前调用仍在同一 Rust 进程，不能充当跨进程恢复联试；外部锁覆盖及完整 WP05/WP06 Gate 仍开放。

## 2026-11-25：WP05 固定原 TC 已提交分支跨进程恢复

子 Rust 进程在原 Java TC 上登记真实分支，确认本地 COMMIT 后直接退出；新 Rust 进程按原 006/004 身份恢复 `Applied`，重新建立 RM 连接并报告原 `PhaseOne_Done`。Rust 1.99/1.85 各通过 1/1，runner 对原帧、预期连接 reset、数据库持久事实及原 TC 回调完成独立核验。TM 在父进程持续运行，故这只覆盖 RM 重建后的已提交分支；TM 重建、COMMIT 结果未知时的固定 TC 联试、跨进程报告一次性持久门闩与外部锁覆盖仍开放。见[双工具链原始证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。

## 2026-11-25：WP05 成功报告跨进程持久门闩

正式原 TC 成功报告与恢复报告现共用 007 `report_started`：新尝试显式置 0，独立控制事务确认置 1 后才发送原 `PhaseOne_Done`；旧 006 行迁移默认置 1，防止把既有报告当成未发送。门闩更新或 COMMIT 未确认时禁止发包，已开始的报告丢包或进程退出后不自动重发。双工具链各 38/38 项普通真实 MySQL 回归通过；固定原 Java TC 的正常、回包丢失、同进程恢复、跨进程 RM 恢复四类场景均双工具链通过，其中跨进程场景再调用得到 `AlreadyStarted`，runner 独立核对 007 行与单次原帧。旧提交竞态夹具初次遗漏新行必需的 `report_started=0`，导致一次安全拒绝；修正夹具后全组重跑通过。证据见[固定原 TC](phase-2/evidence/java-tc-changed-commit-report/README.md)与[MySQL 回归](phase-2/evidence/mysql-commit-attempt-arm/README.md)。007 门闩自身的回包丢失与发包前退出尚未现场注入，TM 重建、外部锁覆盖和完整 WP05/WP06 Gate 仍开放。

## 2026-11-25：WP05 报告门闩 COMMIT 回包丢失

自有 MySQL 代理只截留 007 门闩控制 COMMIT 的原服务端 OK；Rust 1.99/1.85 各 1/1 通过。调用方返回 `Claim` 未知而不发原 TC 报告；独立连接确认门闩已持久为 1，再次调用返回 `AlreadyStarted`，受控对端两次均未收到报告，MySQL 日志只增加一次门闩 COMMIT。原始输出见[门闩故障证据](phase-2/evidence/mysql-commit-attempt-arm/README.md)。这仍是受控 RM 对端，不等于固定原 Java TC 在同故障下完成全局事务；门闩确认后发包前退出、TM 重建、外部锁覆盖与完整 WP05/WP06 Gate 仍开放。

## 2026-11-25：WP05 门闩确认后发包前子进程退出

子进程从原 006/004 身份恢复并持久抢占 007 门闩，随后停在无响应的 RM 注册；父进程确认 `report_started=1` 后终止子进程，新连接重新观察到本地 `Applied`，但报告入口返回 `AlreadyStarted`，受控原对端始终未收到报告。Rust 1.99/1.85 各通过 1/1，同进程取消反例及普通真实 MySQL 回归亦各 41/41 通过；见[跨进程故障证据](phase-2/evidence/mysql-commit-attempt-arm/README.md)。固定原 Java TC 在该窗口的最终事务处理、TM 重建、外部锁覆盖与完整 WP05/WP06 Gate 仍待验证。

## 2026-11-25：WP05 两个恢复调用竞争原成功报告

已提交业务的 RM 子进程退出后，两个独立 MySQL 管理器及连接池同时恢复并争抢原分支 007 门闩。受控 RM 对端与固定原 Java TC 的双工具链现场用例均各通过 1/1：只有一个调用获得发送权，另一个得到 `AlreadyStarted`；原 TC 仅收到一次成功报告并完成单次 GlobalCommit 和回调。未竞争的跨进程联试也双工具链复跑通过。两个竞争者同处父进程；不同恢复者进程竞争、TM 重建、外部写入器锁覆盖及完整 WP05/WP06 Gate 继续开放。证据见[固定原 TC](phase-2/evidence/java-tc-changed-commit-report/README.md)与[MySQL 门闩](phase-2/evidence/mysql-commit-attempt-arm/README.md)。

两个真正独立恢复者进程的启动屏障/原报告帧/数据库门闩测试已加入 `live_commit_preconditions`。最初 D 盘原生 MySQL 受防火墙阻断，不能计为现场回归；后改用 D 盘 Docker 数据镜像、独立 `mysql:8` 容器和回环端口，未修改防火墙。修复受控 RM 对端将败者登记连接误当业务报告的竞态后，Rust 1.99/1.85 各通过 1/1：一个进程报告成功，另一进程得到 `AlreadyStarted`，007 持久门闩为 1，对端只收到一条业务报告。双版本目标严格 Clippy 与格式检查通过，容器已删除。见[跨进程竞争现场证据](phase-2/evidence/mysql-report-process-race/README.md)。该项只覆盖受控 RM 对端；固定原 Java TC 的双进程竞争、TM 重建、外部写入器锁覆盖及完整 WP05/WP06 Gate 仍开放。

## 2026-11-25：WP07 TC 出站端点配置校验增量

`ConnectionPoolConfig::new` 原先接受 `127.0.0.1:0`，测试先实际失败；现于启动连接池前拒绝端口 0、`0.0.0.0` 与 `::` 未指定目的地址。Rust 1.99/1.85 的目标测试与各 87 项 `seata-transport` 回归均通过，两个工具链严格 Clippy 和格式检查通过。此为 WP07 配置验证的一项增量；应用 ID、服务组、部署端点发现、TLS 等整体验收继续开放。

随后登记身份构造器的负向测试先实际暴露空白应用 ID 可被接受；现要求版本、应用 ID 和服务组不能仅含空白或含控制字符，可选附加数据仍按原有 string16 长度边界处理。Rust 1.99/1.85 各 87 项传输回归、严格 Clippy 和格式检查再次通过。此为 Rust 部署配置的显式提前拒绝；原 Java 请求对象本身不施加该校验，正常身份值的线缆表示未改。应用身份的部署来源、认证凭据与完整 WP07 配置 Gate 仍未闭合。

## 2026-11-25：WP07 SQL/Undo 安全边界只读审查

核对业务 SQL、MySQL 动态镜像/补偿 SQL 与 Undo 原字节的主要输入到执行路径；源到汇的校验点和未证明的边界见[审查记录](phase-2/evidence/wp07-sql-undo-security/README.md)。Rust 1.99/1.85 各自的 `seata-at` 非忽略测试与全目标严格 Clippy 通过。记录未签发“无 SQL 注入”或完整安全 Gate：真实 MySQL 模式及元数据竞态、所有格式持续模糊测试、Java TC 故障路径和依赖/部署安全仍需后续验证。

## 2026-11-25：WP07 锁定依赖公告审计

使用 RustSec 数据库快照审计根 `Cargo.lock`，原有 `url 2.5.8` 与 `rsa 0.9.10` 两项公告。将 `url` 锁定到已修复的 2.5.9 后复扫只剩 `rsa`；后者由 `sqlx-mysql 0.8.6` 的非可选依赖引入。所核对 SQLx MySQL 认证路径只见公钥加密，因此私钥计时问题在该路径似乎不可达，但尚未给出完整可达性证明或风险处置。Rust 1.99/1.85 的 MySQL crate 库测试、配置契约测试和严格 Clippy 均通过。原始扫描、依赖链、测试范围与剩余风险见[WP07 依赖审计证据](phase-2/evidence/wp07-dependency-audit/README.md)。WP07 安全 Gate 继续开放。

## 2026-11-25：WP07 Undo 六格式确定性损坏语料

新增跨格式负向测试，对六种 Undo 解析器的默认与非空分支进行截断和逐字节损坏，并对 Fastjson、Jackson、Fastjson2 使用固定 Java 语料；每个原始语料先确认可解码。Rust 1.99/1.85 各 1/1 通过、变异数至少 1000；两工具链完整 `seata-at` 测试和全目标严格 Clippy 均通过，格式检查通过。此项只证明所选确定性损坏输入不引发 panic，持续模糊测试和真实 MySQL Undo 安全验收仍开放。见[WP07 SQL/Undo 审查记录](phase-2/evidence/wp07-sql-undo-security/README.md)。

## 2026-11-25：WP05 TM 重建权限边界核对

检查固定原 TC 跨进程联试与正式 `seata-tm` 源码后确认：当前只重建 RM，TM Launcher 始终保留在父进程；`recover_unknown` 依赖原管理器私有内存凭证，`participant(xid)` 不能结束全局事务。已在[一阶段提交契约](phase-2/mysql-first-phase-commit-contract.md)写明 TM 进程重建所需的持久来源、结束意图、排他接管、状态查询及双进程故障门禁，并在[原 TC 证据](phase-2/evidence/java-tc-changed-commit-report/README.md)补记其适用范围。Rust 1.99/1.85 的 `seata-tm` `manager_calls` 各 13/13 通过，其中包含参与方不得结束全局事务和伪造诊断不得取得恢复权的负向用例。该契约增量未实现新的 TM 持久能力，也未补足固定原 TC 的 TM 重建现场证据；WP05/WP06 完整 Gate 继续开放。

## 2026-11-25：WP07 入站队列帧字节预算

`ConnectionPoolConfig::new` 先前仅检查入站条目数不超过 Tokio semaphore 上限，允许 64 KiB 最大帧配置搭配 1025 项队列；按 WP07 配置要求新增测试并实际看到该边界断言失败。现在构造时以溢出安全的乘法限制最大排队帧字节数为 64 MiB，验证 64 KiB × 1024 和 8 MiB × 8 可用、相邻超限值拒绝，并拒绝最大帧/队列乘法溢出。Rust 1.99/1.85 各 87 项 `seata-transport` 回归、严格 Clippy 和格式检查通过。该预算不等于完整进程内存配额；WP07 部署配置与生产安全 Gate 仍开放。

## 2026-11-25：WP07 预发布功能矩阵与恢复指南

根据现有 WP01–WP04 门禁、WP05/06/07 未闭合项和实验性 MySQL 代码，新增[能力矩阵与恢复操作指南](phase-2/v1-feature-matrix-and-recovery-guide.md)。它区分已验证的初期配置、受限无变化提交、默认关闭的有变化提交实验项和 V1 范围外能力；对原 004/006/007、`Applied`、`NotAppliedFenced`、报告门闩未知及 TM 重建权限给出操作决策和只读查询模板。文档明确完整 V1 尚不可声明生产可用。该指南未经过生产故障演练或 WP07 运维验收，清单项继续开放。

## 2026-11-25：WP07 MySQL 目的地址与用户名预检

`MySqlOptions::new` 原接受 `0.0.0.0`、`::` 作为连接目的地址和全空白用户名。配置契约中的 `0.0.0.0` 负向断言先实际失败；现构造阶段拒绝未指定 IP 与仅空白用户名，不改合法主机名、字面 loopback 或 TLS 默认设置。Rust 1.99/1.85 的 5 项配置测试、45 项库测试（4 忽略）和实验性特性全目标严格 Clippy 均通过，格式检查通过。真实数据库 TLS 握手、身份授权和完整 WP07 配置 Gate 继续开放。

## 2026-11-25：WP07 当前连接代际计数进入池快照

`PoolHandle::snapshot` 原只有槽位状态，当前通道的 pending、迟到响应、写后未知等 `ConnectionStats` 需要先 acquire 通道才能查看。测试先因池槽缺少 `connection` 字段编译失败；现快照按同一 generation 附加当前连接计数，撤回后清空，重连时不把旧计数误归新槽。首次完整工作区回归在大量 MySQL 集成目标链接时耗尽 D 盘空间并出现 `rustc-LLVM`/`link.exe` 写入失败；随后外部释放空间后，未删除构建产物，重新运行 `cargo +1.99.0 test --workspace --offline --locked --quiet`，退出码 0。Rust 1.99/1.85 的完整 transport 测试各 88/88、全目标严格 Clippy 和格式检查通过；真实 MySQL/Java TC 场景在工作区回归中按标记跳过，仍需单独执行。另将跨代累计、防漏计、未知结果语义与敏感字段限制写入[WP07 可观测性实现契约](phase-2/wp07-observability-contract.md)。池计数仍只是当前代际样本，不含跨代累计/exporter；WP07 指标 Gate 继续开放。

## 2026-11-25：TM/RM 所有者只读连接快照

TM/RM 管理器原来无法让外部调用方在不获取业务连接的情况下读取其池状态。按[WP07 可观测性契约](phase-2/wp07-observability-contract.md)添加 `connection_snapshot` 只读入口；各自的真实 TCP 定向测试先因方法缺失编译失败，随后确认注册连接的 Ready 状态、当前代际归属、零 pending 和已完成调用数。Rust 1.99/1.85 的完整 TM/RM 目标测试、全目标严格 Clippy 与格式检查退出码均为 0。此次增量后未重跑完整工作区测试；上条记录的 Rust 1.99 工作区通过结果属于先前代码状态。没有累计导出、结构化日志或真实 MySQL/Java TC 验收，WP07 保持开放。

后续回归尝试：D 盘仅余 266 MB 时，对本次构建目录内 177 个 PDB 使用 NTFS 无损压缩，文件未删除，空间暂恢复到约 3.69 GB。随后重新运行 Rust 1.99 离线完整工作区测试，MySQL 集成目标链接时再次接近满盘，`link.exe` 报 `LNK1318`，命令退出 1；这不是代码断言失败，也不是该代码状态的工作区通过证据。TM/RM 的双工具链定向回归仍为当前有效证据。真实 MySQL/Java TC 测试、完整工作区回归及 WP07 Gate 保持开放。

最终同代码状态的回归改用 E 盘独立构建目录，并设置 `CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_PROFILE_TEST_INCREMENTAL=false` 及 E 盘 TEMP/TMP。`cargo +1.99.0 test --workspace --offline --locked --quiet` 退出码 0，D 盘未再增长。该命令仍按标记跳过真实 MySQL/原 Java TC 集成用例；在该次执行时 Rust 1.85 全工作区尚未重跑，累计指标导出、结构化日志与 WP07 Gate 仍开放。前述 D 盘 `LNK1318` 失败保留为环境事件，不再是本代码状态缺少 Rust 1.99 离线工作区回归的理由。

同样以独立 E 盘构建目录和上述无调试符号配置执行 `cargo +1.85.0 test --workspace --offline --locked --quiet`，退出码也为 0。两工具链离线完整工作区回归现均通过；按标记跳过的真实 MySQL/原 Java TC 用例、累计指标导出、结构化日志及 WP07 Gate 仍开放。

## 2026-11-25：WP07 TM 结构化结果事件

按[可观测性契约](phase-2/wp07-observability-contract.md)为正式 TM 单次 RPC 出口添加 `tracing` 事件，只记录操作、可选 XID/请求编号和 `response`、`unknown`、`not_sent` 静态类别；不把收到 Failed 报文记作业务成功，也不输出完整请求、响应或错误 Debug。真实 TCP 状态查询测试先因无事件失败，补充实现后验证四种情形：正常响应、TC 明确拒绝、写后丢响应和连接前失败。Rust 1.99/1.85 的 TM 目标测试和全目标严格 Clippy、格式检查通过。该事件需要应用配置 subscriber 才能收集；RM/BranchId/资源标识、数据库事件、凭据样本、生产收集与追踪仍未交付。上文两工具链完整工作区通过结果早于本次日志增量，不能代替本次增量的全工作区回归。

此后用各自独立 E 盘构建目录、关闭测试调试信息，重跑 `cargo +1.99.0 test --workspace --offline --locked --quiet` 与 `cargo +1.85.0 test --workspace --offline --locked --quiet`，两者退出码均为 0。真实 MySQL/原 Java TC 场景继续按标记跳过；此通过结果不能关闭 WP05/06/07 或证明生产日志收集和脱敏。

## 2026-11-25：WP07 RM 结构化结果与资源摘要

在 RM 三类出站请求共用的调用出口增加 `rm_rpc_outcome` 事件：操作、可选 XID、请求中的 BranchId、实际请求编号、资源 ID 的完整 SHA-256 摘要和 `response`/`unknown`/`not_sent` 类别。资源原文、应用数据、锁键、请求/响应 Debug 均不作为事件字段。首项真实 TCP 测试先因无事件失败；随后四项测试覆盖正常响应、TC Failed、写后丢响应及连接前失败，校验固定资源摘要、应用数据哨兵不外泄和未分配请求 ID 的 `None`。Rust 1.99/1.85 各 58 项 RM 目标测试通过（另 1 项忽略），全目标严格 Clippy 与格式检查通过。完整凭据型资源 URL、回调/分支状态事件、subscriber/收集端和真实 MySQL/Java TC 验收仍开放；上文工作区通过结果早于本次 RM 增量。

随后分别以 Rust 1.99/1.85 和独立 E 盘无调试符号构建目录重跑 `cargo test --workspace --offline --locked --quiet`，两者均退出 0。真实 MySQL/原 Java TC 用例在离线工作区回归中仍按标记跳过；凭据型资源 URL 的生产脱敏策略和 WP07 日志 Gate 仍开放。

新增第五项 RM 事件测试：用包含虚构用户名、密码和查询令牌的资源 URL 完成原生 RM 登记及分支报告，捕获事件只含预先独立计算的 SHA-256 摘要，未出现 URL、用户名或凭据哨兵。Rust 1.99/1.85 的完整 RM 目标测试各 59 项通过（另 1 项忽略），全目标严格 Clippy、格式检查通过。此测试只覆盖本地 tracing 事件字段，不证明生产 subscriber 的编码/访问控制或下游收集端脱敏；前述工作区回归早于此纯测试增量。

## 2026-11-25：WP07 RM 回调本地决策事件

正式 RM 回调分派在形成每个子请求的本地决定后输出 `rm_callback_decision`：原始外层请求 ID、XID、BranchId、资源 SHA-256 摘要、方向、BranchStatus 与 ResultCode。真实 TCP 测试先因缺少事件失败；实现后提交、重复提交、反向回滚共生成三个可关联决策事件，而资源效果只执行一次。另一项混合无效批次测试确认资源零效果、决策事件零条；事件字段不含资源原文和凭据哨兵。事件发生在回包写入前，因此仅证明本地分派，不证明 TC 收到响应。

Rust 1.99/1.85 的完整 RM 目标测试各 61 项通过（另 1 项忽略），全目标严格 Clippy、格式检查和各自独立 E 盘目标目录中的 `cargo test --workspace --offline --locked --quiet` 均退出 0。真实 MySQL/原 Java TC 用例在离线工作区回归中仍按标记跳过；生产 subscriber、回包送达观测、持久分支状态与 MySQL 事件、日志收集端及完整 WP07 Gate 继续开放。

## 2026-11-25：WP07 连接池跨代累计计数

此前池快照只显示当前连接代际，短连接撤回后无法从轮询者视角恢复其计数。先在断线重连、正常关闭测试中加入累计断言，因缺少字段实际编译失败。现新增 `PoolCounters`，池快照合并已归档、退出中和当前代际；监督器在会话关闭并等待任务后归档，重复撤回不重复计数。额外单元测试模拟尚未发布到可选槽位的短代际，并核对撤回、归档、停止后其心跳计数均为 1。持有旧代际句柄不会让新槽位的当前计数继承旧值。

Rust 1.99/1.85 各 89 项 `seata-transport` 测试、全目标严格 Clippy、格式检查和各自独立 E 盘目标目录中的离线全工作区测试均退出 0。该快照是进程生命周期内的计数，并非持久化或已导出的生产指标；真实 MySQL/原 Java TC 集成用例按标记跳过。WP07 的导出、跨进程连续性、TM/RM/数据库语义计数及部署监控仍开放。

## 2026-11-25：WP07 本机认证指标导出

新增 `seata-observability` 工作区 crate；TM/RM 所有者签发仅可采样的 `PoolSampler`，新组件只绑定回环地址，要求 32–128 字节可见 ASCII Bearer 令牌和至少一个采样源。`GET /metrics` 只输出固定 `tm`/`rm` 角色、配置槽位及固定状态，连接状态/在途为 gauge，池生命周期计数为 counter。HTTP 请求头上限 4096 字节、读取期限 2 秒、并发采集 16；关闭导出器不关闭业务池。使用说明见[crate 文档](../crates/seata_observability/README.md)。

四项配置/真实 TCP 测试覆盖非回环/弱令牌与空采样源启动拒绝、缺失/错误/重复凭据、过长/超时请求头、断线撤回后的累计值以及响应不含令牌、XID、BranchId、资源 URL 等字段。空源负向测试先实际失败，增加启动校验后通过；超长报文的首个测试写法在 Windows 上遇到未读数据导致连接重置，改为恰达上限的未完成请求头后得到 431。Rust 1.99/1.85 的组件测试和 TM/RM/传输定向测试、全目标严格 Clippy、格式检查及最终代码状态的离线完整工作区回归均退出 0。真实 MySQL/原 Java TC 用例在该回归中按标记跳过；生产代理/采集端、TLS、告警、TM/RM 业务语义指标和故障现场仍需独立验收，WP07 保持开放。

## 2026-11-25：WP05 固定原 Java TC 的双恢复进程竞争

恢复固定 Connector/J 8.0.33 的哈希一致夹具，并为原 TC 联试 runner 增加受标签、镜像摘要及回环端口校验的 Docker Desktop MySQL 后端。沿用原 Java pin/JAR、原始 RM/TM 帧观察器和独立数据库查询。业务 RM 子进程确认本地 COMMIT 后退出；父测试在同一 TCP 屏障释放两个不同 PID 的恢复子进程，各自重建 RM/MySQL 管理器，争抢同一 007 `report_started` 门闩，并维持 RM 会话到原 TC 回调结束。

最终代码的 Rust 1.99/1.85 联试各通过 1/1：两个恢复 PID 不同，结果各为一次发送和一次 `AlreadyStarted`；原 TC 只收到一次 `PhaseOne_Done`，回同请求 ID 成功响应，随后一次 GlobalCommit 和一次 BranchCommit 回调及同连接成功响应。数据库终态 `global/branch/lock/business/undo/witness/terminal=0/0/0/1/0/1/1`，007 门闩为 1。两工具链的原单进程双连接池竞争也重新通过；目标严格 Clippy、格式及 Python 语法检查通过。原始帧、PID、数据库查询和清理记录见[固定原 TC 联试证据](phase-2/evidence/java-tc-changed-commit-report/README.md)。自有容器已删除。TM Launcher 仍在父进程，外部写入器锁覆盖及 WP05/WP06 全部门禁仍开放。

## 2026-11-25：TM 跨进程 Launcher 来源表的首个结构门禁

继续[TM 持久恢复契约](phase-2/tm-durable-launcher-design.md)，新增独立于 RM 006/007 的 008 `seata_rs_tm_launcher_v1` 表草案。原 XID 使用全字节 `MEDIUMBLOB` 与 SHA-256 摘要索引，保留 TC 端点、应用/服务组、Begin 附加数据、业务意图、期限、拥有者代际、结束操作及发送门闩列；CHECK 约束拒绝空 XID、非法操作和无意图已发送等形状。结构测试先在缺约束草案上实际接受非法行而失败，补充约束后 Rust 1.99/1.85 各在隔离 MySQL 8.4.13 库通过 1/1。两条独立连接竞争意图与发送门闩 CAS 时各仅一方影响一行；目标严格 Clippy、格式检查通过，容器已删除。见[红绿灯与边界](phase-2/evidence/tm-launcher-schema/README.md)。008 表尚未接入正式 TM；旧拥有者停止证明、真实 Begin 来源写入、结束帧故障及固定原 TC 的 TM 重建仍待实现，WP05/WP06 门禁保持开放。

## 2026-11-25：TM Begin 来源与结束发送门禁接入

正式 TM 新增 `begin_durable`：Begin 成功后在应用获得 Launcher 前持久写入 008 来源，写入失败则不交付句柄；结束操作先持久确认意图，再以 CAS 置位不可清除的发送标记，任一写入失败均不发二阶段帧。真实 TCP 用例先因缺少 API 失败，补实现后通过来源拒绝和发送标记拒绝用例；独立 MySQL 8.4.13 与 TCP 联试在 Rust 1.99/1.85 各通过 1/1，观察到原身份字段、保守期限和 Commit 帧到达前可见的门闩。严格目标 Clippy 与全仓格式检查通过，见[现场证据](phase-2/evidence/tm-launcher-store/README.md)。跨进程接管、旧拥有者停止证明和原 Java TC 结束帧故障仍未完成，WP05/WP06 保持开放。

## 2026-11-25：TM 门闩后退出与固定原 Java TC 回包丢失

在自有 MySQL 008 表中，独立 TM 子进程于发送门闩提交后、返回网络发送路径前立即退出；Rust 1.99/1.85 各通过 1/1，父进程受控对端未收到结束帧，来源保留 `end_operation=1,send_started=1,owner_epoch=1`。受控对端收到 Commit 后丢响应的双版本用例也各通过 1/1。再把同一持久 TM 路径接入固定原 Java TC 与认证帧代理：Commit/Rollback 两种操作在 Rust 1.99/1.85 四组联试各通过 1/1，原 TC 每组实收一次结束请求、同请求 ID 成功回包被截留、客户端返回未知且旧内存恢复入口被拒、008 门闩仍为 1；代理无第二次结束请求，进程清理均成功。见[原始帧、数据库与清理证据](phase-2/evidence/tm-launcher-store/README.md)。两工具链严格 Clippy、格式及 Python AST 检查通过。跨进程受信接管、其他故障状态和完整 WP05/WP06 门禁仍开放。

## 2026-11-25：TM 008 来源只读核验

在既有持久恢复设计补充只读来源快照契约；正式 MySQL 存储按 SHA-256 摘要查询后强制核对完整原 XID，校验端点、登记身份、原 Begin 字段、结束状态与拥有者代际，期限由 MySQL 服务端时钟判断。缺 API 的反例测试先编译失败；实现后，真实 MySQL 的缺记录/同摘要异 XID 冲突用例和真实 Begin→Commit→期限过期快照用例均在 Rust 1.99/1.85 各通过 1/1。快照只供观察，不授予全局结束或代际更新权；见[现场证据](phase-2/evidence/tm-launcher-store/README.md)。旧拥有者停止证明和 TM 跨进程接管继续开放。

## 2026-11-25：TM 009 实例绑定与旧行失败关闭

正式 TM 构建时生成 32 字节随机实例 ID，Begin 来源写入该 ID，原意图与发送门闩 CAS 均须匹配，快照提供只读核验。新增 009 增量迁移，保留 008 的旧行并令其 ID 为 `NULL`；旧行无法通过新 CAS 或快照核验，不会凭空获得接管权。真实 MySQL 的 008→009 旧行迁移、不同实例 CAS 拒绝、同摘要异 XID 拒绝、Begin→Commit 标记与进程退出后数据库 ID 对应测试在 Rust 1.99/1.85 均通过。模拟 MySQL 标记已提交而存储层确认丢失时，没有结束帧。固定原 Java TC 的 Commit/Rollback 成功回包丢失四组也按最终迁移路径重新通过；见[详细证据](phase-2/evidence/tm-launcher-store/README.md)。双工具链目标严格 Clippy、格式检查和相关 TM/MySQL 单元测试通过。实例 ID 仍不是旧进程停止的可信证明；接管 API、生产数据库权限与 WP05/WP06 完整验收继续开放。


## 2026-11-26：应用事务准入与有界排空

沿用 WP07 停机契约，正式 TM crate 新增共享 TransactionAdmission 与独占 TransactionPermit，拒绝关闭后的新作用域，并等待旧作用域释放。超时只返回数量，不取消业务、不重新开放入口、不自动改写事务结果。真实 TCP 测试在原 Commit 响应被对端屏障持有时验证排空超时，随后原作用域完成并关闭 TM。缺 API 的测试先失败；删掉释放通知的变异版本在已挂起等待者断言失败。Rust 1.99/1.85 各通过 47 项组件回归和 1 项接口示例，另 7 项外部环境测试保持 ignored；严格目标 Clippy 通过。证据见[应用准入验收](phase-2/evidence/transaction-admission/README.md)。生产应用仍须统一接入、绑定信号并验证部署；WP07 总项和全部计划继续开放。


## 2026-11-26：可选应用停机信号与 Windows 原生事件

在 WP07 契约下新增 application_signals 可选功能，由应用显式安装 ApplicationSignals 接收 Windows Ctrl-C/Ctrl-Break 或 Unix SIGINT/SIGTERM。默认 Tokio 功能图无 signal；接口不会隐式消耗 TM/RM 所有者。Rust 1.99/1.85 各在独立隐藏控制台接收真实 Ctrl-C 和 Ctrl-Break；原 Commit 响应仍被屏障持有时，关闭准入、排空超时且原作用域和通讯所有者仍有效。各例再次接收同种事件后仍保持原作用域，控制器放行原响应后仅一次 Commit、排空完成及 socket EOF。把 Ctrl-Break 错映射为 Ctrl-C 的变异现场被实际断言拒绝。两工具链各通过 47 项组件回归和 2 项接口示例，全目标严格 Clippy 与默认功能构建通过；8 项环境相关测试在普通回归中保持 ignored，其中新增信号用例已由外部运行器显式执行。见[原生事件证据](phase-2/evidence/application-signals/README.md)。Unix 实测、完整业务入口覆盖和真实部署验收仍开放，整体计划未完成。


## 2026-11-26：TM 调用结果计数与原 TC 故障路径

沿用 WP07，正式 TM 新增仅能采样的固定操作计数能力，区分有效权威响应、TC Failed、未知、明确未发送、本地拒绝和取消，保留 active 及协议错误附加计数；观测不改写原结果，不发起重试。现有回环 Bearer 导出器增加固定 32 条 TM 序列，兼容原连接来源，关闭后可读取最终值。5 项 TCP 用例、真实认证 HTTP 和并行饱和边界验证通过，首次缺 API 与缺导出入口的失败记录保留。

固定原 Java TC 与真实 MySQL 的 Commit/Rollback 成功响应丢失四组现场由 Rust 1.99/1.85 通过：原管理器 Begin observed=1、原 End unknown=1；新管理器只读原 XID，Status observed=1。原帧、原门闩与全部固定计数字段独立核对，各五个削弱反例均被拒绝。1.85 首轮 TC 因端口占用退出，确认清理后重跑通过，不计为客户端行为证据。两个版本各通过全工作区全功能 690 项回归，313 项需外部环境的用例保持 ignored；四组原 TC 是另外实际执行的现场，不能替代其余 ignored。TM、observability、MySQL 全目标严格 Clippy 通过。详见[TM 调用指标证据](phase-2/evidence/tm-call-metrics/README.md)。完整事务存活、RM 分支与 MySQL 状态指标、重试、生产代理/采集/告警以及 WP05–07 总门禁仍开放。


## 2026-11-26：RM 日志状态、分派事件与原 TC 停机计数

新增仅持有固定原子计数的 RmMetricsSampler / RmBranchMetrics，读取同一日志打开周期的保留状态 gauge 与分派结果、复用、错误、取消和 apply/recover 尝试 counter。同步索引后才更新 gauge，重开从原记录重建；旧采样器不保留文件锁，重开后已有非零分派累计归零。回环认证端点增加固定 34 条 RM 序列。缺 API 和缺导出入口先失败；将状态发布移动到真实写入/同步之前的变异在只读 OS 句柄故障用例中失败，恢复后通过。

Rust 1.99/1.85 的四组原 Java TC/MySQL 现场通过：原物理 COMMIT 在 RM 关闭前后仍在执行，Pending 保留；旧调用取消，新会话 recover 确认后只同步一次 Completed，资源 apply/recover 各一次。额外自然重叠现场有两个旧回调等待同一分支，取消数为 2 而 apply 仍为 1。修正验收脚本固定回调基数的假设，改用实际原始帧计数和完整身份约束。该额外现场还保留 15.039 秒后返回的 Failed/RollbackRetrying，原 TM 的 10 秒请求已保持 Unknown；原帧重放检查关联、时序和后续只读 Finished，并拒绝四种迟到失败证据削弱。其初轮未执行外部最终数据库计数，作为诊断重放保留，不冒充第五次完整现场成功。

双版本全工作区全功能各 697 项通过、313 项 ignored；最后加强日志重开累计归零断言后，双版本 journal 目标测试及 RM/observability/MySQL 全目标严格 Clippy 另行通过。最终四组现场、九种计数/原连接反例及源代码写前误发布变异见 [RM 指标证据](phase-2/evidence/rm-branch-metrics/README.md)。RM 出站结果、完整 MySQL 本地状态、生产采集/告警和 WP05–07 全部门禁继续。

## 2026-11-27：RM 出站调用结果与原 TC 三类响应

正式 RM 新增管理器/客户端克隆共享的只读 RmCallMetricsSampler，三种操作区分有效成功、TC Failed、未知、未发送、本地拒绝和取消，维护 active 与协议错误计数；注册回执只计一次，成功 LockQuery false 仍属于 observed。认证 HTTP 增加固定 24 条序列，无业务身份标签，保持 Java 可选字段和原重试策略。

Rust 1.99/1.85 的正常提交、报告成功回包丢失、Finished XID 注册拒绝共六组固定原 Java TC/MySQL 现场全部通过，原帧实际投递与固定计数逐字段核对，各五个证据削弱反例均拒绝。双版本全工作区全功能各 705 项通过、314 项 ignored；RM/observability/MySQL 全目标严格 Clippy 通过。重复回执源代码变异使 observed=2 的目标测试实际失败，恢复后通过。三次验收脚本诊断失败单独保留，不计完整现场成功。见 [RM 出站指标证据](phase-2/evidence/rm-call-metrics/README.md)。WP05–07、完整 MySQL 状态、生产采集告警和整体计划继续开放。

## 2026-11-27：MySQL 恢复控制事务计数与确认丢失

新增同一数据源全部克隆共享的 MySqlRecoveryMetricsSampler，六种内部控制动作按 completed、unknown_commit、before_commit_error、cancelled 结算，成功结果额外区分 Applied/Unknown；重复读取终态计观察，不计业务效果。显式 mysql 导出特性增加固定 42 条序列，默认导出器无 MySQL/SQLx 运行依赖。

双版本真实 MySQL 各 2 项控制用例、1 项认证 HTTP 现场通过，取消前从 InnoDB 核实锁等待。移除结算的源代码变异在真实数据库目标断言失败，恢复后通过。四组固定原 Java TC/MySQL Commit/Rollback COMMIT OK 丢失现场通过：unknown_commit=1，原 TC 重试后读取终态，只产生一次实际数据库效果。自然重叠回调暴露旧验收脚本的额外串行假设，修正为逐请求关联与因果顺序；诊断原帧和八项反例重放保留，不冒充额外完整现场。

Rust 1.99/1.85 全工作区全功能各 707 项通过、317 项 ignored；双版本全目标严格 Clippy、默认功能严格 Clippy 和默认依赖检查通过。见 [MySQL 控制指标证据](phase-2/evidence/mysql-control-metrics/README.md)。一阶段与独立 Undo 全指标、生产日志/采集/告警以及 WP05–07、后续 Phase 仍开放。

## 2026-11-27：原生影响行响应与空镜像故障

补齐 WP05 已记录的真实驱动负向缺口：固定 MySQL 实际零行 UPDATE/DELETE 后，中继只把目标 OK 的 affected_rows 由 0 改为 1。Rust 1.99/1.85 默认构建各通过有无先前未提交 INSERT 的四种组合；镜像错误后 DML 与准备均 Poisoned，原 RM 零分支请求，MySQL 零本地 COMMIT。原目标物理会话全部在服务端消失后，独立 SQL 证明先前写入整体回滚，Undo/控制记录为空，原 TC 四个 XID 只经一次回滚及只读观察收敛 Finished。

移除原生产保护的真实驱动变异返回 Ok(1) 并触发目标失败；finally 恢复原字节后四例完整重跑通过。三个完整现场的保留原帧各拒绝十六种最终证据削弱；额外默认无变化提交对照通过。双版本全工作区全功能各 707 项通过、318 项 ignored，MySQL 全目标全功能严格 Clippy 通过。首轮异步 TC 状态/驱动屏障诊断独立保留，不计成功。见 [原生影响行证据](phase-2/evidence/mysql-affected-image-native/README.md)。本轮补充故障证据，不修改生产保护；公开有变化提交、完整外部写入器、WP05–07 及后续 Phase 仍开放。

## 2026-11-27：RM 元数据原测试逐案对照与可重放依赖

从固定 Git 重新编译 ColumnMeta、IndexMeta、IndexType、TableMeta 四个生产类及 RM 对应四个完整原测试类。原 JUnit 十例通过，独立观察副本委托原 JUnit 并捕获全部 51 处断言；Rust 1.99/1.85 在四个独立测试目标中逐案例保留相同实参顺序、异常和对象变更，生产元数据源码保持原字节。三个实际源码错误变体被目标断言拒绝，恢复后十例通过；每版本另拒绝八种证据削弱。

旧完整 JUnit 基线的物理目录及原输入快照缺失，不能据历史账本声称当前已重验全集。按后续封存摘要找到 930/1079 项相同缓存输入，选定运行/编译闭包的十三项约 4.65 MB 已重新封存并以可信清单锚定后重放。全量历史计数不机械累加，本轮仅新增十例可核验匹配。独立程序只读复核通过，独立人工/另一 Agent 审查门禁仍开放。双版本全工作区各 717 项通过、318 项 ignored，目标严格 Clippy 通过。见 [原案例与依赖证据](phase-2/evidence/rm-metadata-source/README.md) 和 [十例增量账本](phase-2/wp05-rm-metadata-source.json)。其余原案例、完整基线重建及 WP05–07、后续 Phase 继续。


## 2026-11-27：原 Maven/JUnit 基线重新执行

固定 Java Git 原字节 4,269 文件重新导出，24 模块原 Maven reactor 测试编译成功。原 Jupiter/JDK17 实际运行 1,288 例通过、39 例原注解禁用，运行时闭合 1,327 个具体案例及 1,135 个声明方法引用；34 项事件/子 JVM 证据削弱全部拒绝。三子 JVM 压力参数未降低。原源码及 POM 编译和执行前后保持原字节。见 [新原 Java 基线](phase-2/evidence/source-junit-rebuild/README.md)。

本批不改 Rust 源码或全量匹配计数。AST 声明清单、完整运行依赖搬移重放、独立外部审查、其他 JDK profile 与剩余 Rust 案例仍待完成；旧缺失现场未据此宣称恢复。WP05–07、V1 和后续 Phase 保持开放。


## 2026-11-27：原测试 AST 总账与完整输入便携重放

独立 JDK AST 从固定原字节的 189 份 Java 测试重建 1,135 个声明，全部绑定到原 JUnit 的 1,327 个具体案例；七份原资源与 Rust 夹具副本一致。5,623 项约 89.9 MB 原输入封存为约 64.4 MB 归档，完整保留 98 项原运行 JAR 及类路径来源。新 D 目录只使用封存输入及固定 JDK，原 JUnit 1,288 例通过、39 例原注解禁用；37 项证据削弱拒绝，真实三子 JVM 压力未降低，六个 JVM 退出和原 H2 清理核实。

重放暴露观察器字典覆盖同名类历史的问题；保留诊断，并通过目标失败/修复及六项工具回归验证修正。最终现场、方法/案例总账及可信摘要见 [当前便携基线](phase-2/source-junit-rebuild-portable.md)。本批 Rust 源码未改，未重跑或重算全局 Rust 测试/匹配总数。独立外部审查、其他 JDK profile、剩余 Rust 原案例及 WP05–07、后续 Phase 仍开放。


## 2026-11-27：MySQL Undo 执行器原六例与正式补偿路径

提取三类正式补偿执行器的 SQL 构建/镜像选择方法，SQLx 真实执行及 AbstractUndoExecutor 分派共用这些方法。固定原三个测试类六例、13 处断言在双 Rust 逐项匹配；保留三个原输入均为 UPDATE、JDBC CHAR、KeyType、原字符串及对象镜像身份。

双版本各五组真实 MySQL 补偿通过，覆盖复合 PK 与绑定顺序、反序日志、脏数据整体回滚及已恢复跳过。三个源代码变异目标失败；正式 SQL 构建器变异触发真实 MySQL 1064，恢复后同目标通过。原 Undo/物理会话归零，自有容器清理。全工作区全功能各 723 passed / 318 ignored，MySQL 严格 Clippy 均通过。证据、诊断与审查边界见 [Undo 执行器原案例](phase-2/evidence/mysql-undo-executor-source/README.md)。独立外部审查、其余原案例、WP05–07 及后续 Phase 保持开放，不累加为完整对象或全局迁移已完成。


## 2026-11-27：独立 MySQL Undo 调用指标

正式 rollback_undo_log/fence_undo_log 现在按数据源克隆共享只读计数，未轮询 Future 不计；完成、未知 COMMIT、提交前错误、取消分别结算。成功观察与明确返回的恢复行数另计，未知结果即使在服务器实际完成补偿也不计成功。可选 mysql 导出增加固定 20 条序列，不带身份/SQL/凭据标签，默认运行依赖无 MySQL/SQLx。

双版本各六组真实 API/等待/HTTP 现场通过，四次原 MySQL COMMIT OK 被截留；每版本 12 项原证据削弱拒绝。移除正式结算在真实目标计数断言失败，恢复原字节后通过。两个版本全工作区全功能各 726 passed / 320 ignored，MySQL 和 observability 全目标严格 Clippy 通过，自有容器清理。详见 [独立 Undo 指标](phase-2/evidence/mysql-undo-metrics/README.md)。一阶段业务指标、生产日志/采集/告警、独立审查、WP05–07 及后续 Phase 仍开放。


## 2026-11-27：一阶段编排分层指标

正式准备、提交前检查、无变化提交、实验有变化提交和成功报告入口增加数据源共享的只读指标，固定 31 条序列。已准备、见证暂存、本地提交确认、尝试登记不确定和报告未知分别结算；不相加作为业务效果次数。

Rust 1.99/1.85 各 14 组真实 MySQL/HTTP 及六组固定原 Java TC 场景通过，覆盖实际登记 ACK 和报告 ACK 丢失；正式结算源代码变异目标失败、恢复后通过，八项证据削弱拒绝。两版本全工作区全功能各 729 passed / 323 ignored，受影响两组件严格 Clippy 和默认依赖检查通过。证据见 [一阶段指标](phase-2/evidence/mysql-phase-one-metrics/README.md)。业务 DML/显式回滚及完整失败/恢复报告指标、生产日志采集告警、独立外部审查、WP05–07 与后续 Phase 保持开放。


## 2026-11-27：RM 原代际回调响应写入事件

正式回调以原信封编号和连接代际记录单次本地响应写入，区分完整本地写入、明确未发送、写后未知和作用域取消，不把本地写完成称作 TC 接收。分支决策也保留原代际，批次多个决策只对应一次信封写入。

双 Rust 的真实 TCP 批次/重复/反向/旧代际关闭和作用域分类测试通过；固定原 Java TC 各三场景核对普通提交和 Commit/Rollback 回包实际丢弃后的新连接重试，每版本九项证据削弱拒绝。移除正式结算目标失败，恢复原字节后通过。两版本 RM 各 77 passed / 1 ignored，全工作区各 732 passed / 323 ignored，受影响 RM/MySQL 严格 Clippy 和格式检查通过。见 [本地写入证据](phase-2/evidence/rm-callback-write-events/README.md)。生产采集、完整状态事件、独立审查、WP05–07 和后续 Phase 保持开放。


## 2026-11-27：Seata 传输 TLS 1.3 双向认证

正式单连接、连接池及 TM/RM 构建器新增默认关闭的显式 mTLS 入口；证书/SAN 校验、原绝对握手期限及 TLS 重连均无明文回退。写入还需在同一期限内 flush 加密/缓冲记录，不能在接受明文时提前发布完成；确定性受阻 flush 先 RED 后 GREEN。

双 Rust 各八组真实 TLS 目标、27 传输库测试及三组原 Java TC/MySQL 场景通过；会话证书摘要、协商协议及字节总数逐连接绑定原 V1 帧。两项正式源代码变异各失败，恢复后通过；每版本九项证据削弱拒绝。全工作区全功能各 742 passed / 324 ignored，四组件严格 Clippy、默认特性测试/严格检查和默认传输依赖检查通过。见 [传输 TLS 证据](phase-2/evidence/transport-tls/README.md)。这是同主机 TLS 终止代理 profile，不声明 Java TC 原生 TLS 或完整生产认证；证书运维/代理部署授权、独立审查、WP05–07 和 Phase4–7 继续。


## 2026-11-27：持久 TM 来源绑定 TC 传输信任

修复真实 TLS 来源可被同端点明文替代者提供伪造状态的问题。迁移 014 为新 TLS 来源保存版本 2 的根集合/SAN/profile 摘要；查询与持锁接管/恢复发送 CAS 检查同一绑定。旧明文与无 014 schema 保持兼容；新 TLS 缺迁移时保留原 XID，来源失败不交付业务 Launcher。

双版本各五组真实 TLS/MySQL 用例和两组原 Java TC 丢结束回包场景通过，覆盖真实监控子进程停止、行锁改写竞态拒绝及恢复原字节后同来源完成。三个源变异每版本目标失败，原字节恢复后通过；独立程序校验拒绝 native 每版本 12 项、Java 每 profile 12 项证据削弱。全工作区各 743 passed / 330 ignored，四组件严格 Clippy 与默认配置检查通过。见 [来源信任证据](phase-2/evidence/tm-transport-binding/README.md)。独立外部审查、受控信任迁移、生产证书运维、WP05–07 与 Phase4–7 继续。


## 2026-11-27：业务 DML 与显式本地回滚指标

正式 execute 与显式 rollback 入口新增数据源共享的结果计数，未提交的 DML 成功、原错误、回滚确认/未确认及取消分开，Prepared/Checked 转发仅底层结算一次。未轮询与自动 Drop 不计显式 API。原 31 条导出保持语义，新增八条形成 39 条固定序列。

双版本各五组真实 MySQL 和一组授权 HTTP 通过，涵盖真实锁等待/取消/期限、重复键、中毒作用域、代理截留回滚响应及原行锁已释放。每版本两种正式源结算变异失败、恢复后现场通过；12 项证据削弱拒绝。全工作区全功能各 743 passed / 335 ignored，MySQL/observability 全目标严格 Clippy 通过。见 [业务与回滚计量](phase-2/evidence/business-metrics/README.md)。失败报告完整计量、结构化数据库事件、生产采集/告警、独立外部审查及剩余 WP05–07/Phase4–7 继续。

本增量的双版本默认测试/严格检查及默认导出器无 SQLx 依赖验证也已通过。


## 2026-11-27：失败报告的分层计量

正式回滚后报告、原准备错误报告和组合准备/报告入口新增数据源共享的阶段计数；保留本地回滚事实、报告未知、明确错误、无分支、不适用及已尝试门闩的边界。原错误携带只读计数能力，关闭后可观察且不持有数据库池。旧 39 条序列保留语义，当前为 63 条；实验入口仍默认关闭。

双 Rust 各八个真实 MySQL 目标与认证 HTTP 通过；固定原 Java TC 每版本八场景完成失败报告、Undo/归属错误、真实唯一键冲突及成功原回包丢失。独立原帧校验关联 TC 签发分支及原可选字段，每 profile 拒绝 12 项证据削弱。三个正式结算变异每版本失败，原字节恢复后目标通过；全工作区各 743 passed / 340 ignored，受影响两组件全功能及默认严格 Clippy/测试通过。见 [失败报告计量证据](phase-2/evidence/failure-report-metrics/README.md)。一次原代理端口绑定失败及仅限测试助手的单行 lint 修订均有原记录和精确边界。生产事件/采集/告警、独立外部审查、WP05–07 和 Phase4–7 继续。


## 2026-11-27：MySQL 恢复控制结构化结果事件

正式六种控制动作新增单次结果事件，关联原身份、资源 SHA-256 和请求方向；区分返回结果、实际 COMMIT 尝试位与取消，既有终态读取不当作新增业务效果。不记录资源 URL、SQL、Undo、凭据或原错误，不增加数据库操作。

双 Rust 各三个真实数据库目标、两次原 COMMIT OK 截留及随后同分支恢复通过；原 Java TC 各正常 Commit 和两方向回包丢失三场景完成，事件与停止后控制计数及原回调帧逐项关联，每 profile 12 项证据削弱拒绝。三种正式源反例每版本均失败，恢复后目标通过。最终所有生产/夹具源输入均匹配当前字节，全工作区各 744 passed / 343 ignored，受影响两组件全功能/默认严格检查与默认依赖检查通过。见 [控制结果事件证据](phase-2/evidence/mysql-control-events/README.md)。其余数据库事件、生产 subscriber/采集/告警/追踪、独立外部审查、WP05–07 与 Phase4–7 继续。


## 2026-11-27：独立 MySQL Undo 结构化结果事件

独立补偿与围栏入口新增单次原身份/资源摘要结果事件；返回观察和原实际 COMMIT 尝试位分开，已有围栏与 Normal 观察保持只读，未知/取消不推断服务端效果。不记录 SQL、Undo、URL、凭据或原错误，不修改持久格式和恢复权限。

双版本各三个真实 MySQL 目标、两个 API 共四次原 COMMIT OK 截留及同身份后续观察通过；四种正式源反例每版本均失败，精确恢复后最终现场通过，每版本十一项证据削弱拒绝。共享日志捕获模块重复加载的 Clippy 诊断修正后，最终全工作区各 746 passed / 346 ignored、受影响两组件全功能/默认严格检查通过，默认导出器无 SQLx 运行依赖。详见 [独立 Undo 事件证据](phase-2/evidence/mysql-undo-events/README.md)。本轮不新增原 Java TC 结论；其他状态事件、生产采集告警追踪、外部审查和剩余 WP05–07/Phase4–7 继续。


## 2026-11-27：有界结构化日志订阅与 JSON 交付

新增默认关闭的 structured_logs 适配器，七种正式事件按静态字段集合先拒绝再格式化，固定值校验与两级字节预算保护完整 JSON；事件只非阻塞入队，六种只读累计区分入队、取给消费者和四类丢弃。消费者显式关闭后排空，不把入队当作落盘或 TC 接收。

双 Rust 各十个目标及一项真实 MySQL 接入通过，涵盖实际 TM/RM 成功、拒绝、回包丢失、重复/反向回调一次效果与独立 SQL 围栏。原 JSON 写入自有文件、sync_all 后回读；四类正式源反例每版本均失败，精确恢复后通过，每版本十五项证据削弱拒绝。最终全工作区各 757 passed / 347 ignored、观测组件全功能/默认/仅日志严格检查与依赖检查通过。证据及原型/脚本标签诊断见 [有界日志适配器](phase-2/evidence/structured-logs/README.md)。本轮不新增原 Java TC 结论；生产 sink、保留、远端认证采集告警、现有应用日志组合、丢弃端点计量、跨进程追踪、外部审查及剩余 WP05–07/Phase4–7 继续。


## 2026-11-27：结构化日志计数接入认证指标端点

只读日志 sampler 现可独立或与原 TM/RM/MySQL 来源组合，通过原回环 Bearer 端点输出六条固定序列；采集不消费/保留队列，不增加日志递归、身份标签或恢复权限。入队、取给消费者与四类丢弃各自保留语义。

双 Rust 各三个真实 HTTP/TM TCP 目标通过，含错误令牌、全部计数分离、关闭后最终样本、指标源不延长队列寿命及原 32 条 TM 指标组合。三类正式源反例每版本均失败、精确恢复后通过，每版本九项证据削弱拒绝。最终全工作区各 760 passed / 347 ignored，观测组件全功能/默认/仅日志/仅 MySQL 严格检查及默认依赖检查通过。证据、计数分离补强及执行前脚本语法诊断见 [日志指标接入](phase-2/evidence/structured-log-metrics/README.md)。本轮不新增原 Java TC/MySQL 现场；生产抓取告警部署、应用日志组合、追踪、其他状态事件、外部审查和剩余 WP05–07/Phase4–7 继续。


## 2026-11-27：与原应用日志栈组合的可选 Layer

新增默认关闭的 structured_log_layer，复用原日志队列/预算/计数，通过官方 per-layer filter 与底层含 Registry 的原应用栈组合，不全局替换过滤或把应用 span 字段带进 Seata JSON。默认及原仅日志依赖边界保持。

双版本各五个目标及一个实际 MySQL Layer 接入通过：动态过滤按原栈基线对照，原 span 父子/record/follows/克隆/关闭、应用全局过滤、七模板及显式 instrument 的实际 TM 请求均核对。三类正式源反例每版本失败、原字节恢复后通过，每版本十三项证据削弱拒绝。最终全工作区各 766 passed / 347 ignored、观测组件全功能与四种子特性严格检查通过。三个新发布依赖及实际解包输入封存，无关锁边恢复。原型中绝对 Span 状态假设和静态/动态过滤夹具问题见 [Layer 组合证据](phase-2/evidence/structured-log-layer/README.md)。原 Java TC 未重跑；自动线程/进程传播、生产 sink/采集告警、其他状态事件、外部审查与全部后续计划继续。


## 2026-11-27：WP07 正式单文件日志输出器

沿用批准的观测契约，新增默认关闭的 structured_log_file。调用方显式提供独占空文件能力，原字节 write_all/flush 后累计完整写入，真实 sync_all 后累计同步；具备总字节上限、条数/周期/最终同步、脱敏阶段错误及未知尾部标记。关闭停止接纳并排空，Pending/取消等待保留原 JoinHandle 与 I/O，不自动重启/截断/重放；Drop 只请求停止，进度快照不作为完成证明。

双 Rust 各 10 项原生文件、4 项受控任务、1 项真实 MySQL 场景通过；全工作区各 780 passed / 348 ignored（164 个成功摘要含 rustdoc），严格 Clippy 与默认/文件依赖边界通过。真实 Windows 只读空文件系统同步错误、原 MySQL DDL/fence/关闭池 Unknown 和生产文件回读已核对；五类源码反例 × 双版本共十次拒绝，每版本十项证据削弱拒绝，最终归档源码字节与当前一致。Cargo.lock 未改变。[组件 ledger](phase-2/wp07-structured-log-file.json)、[运行与诊断证据](phase-2/evidence/structured-log-file/README.md)。

本增量只关闭选定单文件输出器的实施项，未完成轮转/保留、部署权限、跨进程采集/追踪、断电/目录持久化及外部审查；完整 WP07 和 Phase 0–7 保持未完成。本轮不重复请求已授权的提交和 GitHub 推送。


## 2026-11-27：Phase 4 XA 标识与原生 MySQL 两阶段

新增独立 seata-xa 身份模型和 seata-db-mysql 可选 xa。格式 9752/UTF-8/原分支前缀与固定 Java 源对象实际输出逐字节对照；原生独占作用域支持单次参数化 DML、END/PREPARE、显式原身份 Commit/Rollback 和有界 XA RECOVER。失败/取消封闭作用域，连接退出即退休；NotSent、命令未知与只读观察错误分开，不用缺 Prepared 或 XAER_NOTA 猜终态，不自动重放控制命令。

双 Rust 各 5 项身份、8 项真实 MySQL 场景通过，包含三种真实 OK 截留（各控制命令一次）、先观察实际行锁等待再取消、原进程退出、同一 MySQL 重启和新进程同身份恢复。全工作区各 785 passed / 356 ignored（169 摘要含 rustdoc），最终受影响目标及默认/仅 XA 严格检查通过；原生产字节未变，最后 native 观察补强后已重新执行全部现场/封存。七类生产源反例共十四次拒绝，每版本十二项弱化证据拒绝，独立 SQL/原帧/重启三分量均核验。[XA 契约](phase-2/mysql-xa-contract.md)、[native ledger](phase-4/mysql-xa-native.json)、[证据与诊断](phase-4/evidence/mysql-xa/README.md)。

此为总计划中独立 XA 路径的基础增量，未关闭 XA Resource/RM/Java TC 全局门禁；AT 的外部写入器/TC 锁安全及默认有变化 COMMIT 门槛保持。持久 XA 所有权/方向、正式回调权威恢复、完整故障/SQL/部署门禁，TCC/Fence/空回滚/悬挂和全部 Phase 5–7 继续。

## TCC 真实写入进程与同库恢复增量（2026-11-28）

按现有 Phase 4/TCC contract 补齐双 Rust 的九个真实写入者强杀边界，分别联测数据库持续运行、正常重启与 SIGKILL 后同目录恢复，共 54 场景。独立 OS 句柄、原 COMMIT/OK、实际行锁、原 UUID/Fence/业务、旧日志 CRC 与恢复前后 DML 联合验证通过；16 项 MySQL、4 项正常原 Java TC、8 项原 TC 回调故障回归通过，902 项证据削弱拒绝。工作区各 791 passed / 384 ignored，严格检查通过。[证据](phase-4/evidence/tcc-managed/PROCESS_DATABASE_RECOVERY.md)、[ledger](phase-4/mysql-tcc-process-database-recovery.json)。生产 API、迁移、Java Undo/旧 AT/XA 记录未改动。

该进程矩阵使用正式 RM 与原 V1 协议测试对端，不计作原 Java TC 同时崩溃联合验收。原 TC 一次自然旧连接重试重叠已有严格关联/时序及无业务重放证据。动态上下文报告、完整故障/并发/拓扑、生产权限/保留清理/负载/运维以及 AT/XA 剩余门禁、Phase 5–7 仍未完成，范围不缩减。

## TCC 动态上下文的核心依赖（2026-11-28）

按既有 TCC contract 实现受控参数变更、可选 updated/delayReport 和初始化后跟踪。固定 Java 执行 46 项观察，Rust 核心比较其中 45 项；另 1 项 util 自动报告观察继续保留为后续基线。原始 RawValue 数值不重编码，Double 仅用于 Java 对象相等性的兼容比较；updated 不是报告/事务成功或原字节相同的证明。[证据](phase-4/evidence/tcc-core/CONTEXT_MUTATION.md)、[ledger](phase-4/tcc-context-mutation.json)。

双版本各 795 passed / 384 ignored，严格检查通过；17 类错误源码每版实际行为失败，共 34 次。当前源码重新执行 54 项进程/数据库恢复、16 项 MySQL、4 项正常原 Java TC 和 8 项原 TC 故障事务，924 项证据削弱拒绝。原 Undo 格式、AT/XA 数据和默认 TCC 业务接口未改动。可变 Try 业务上下文、立即/延迟网络报告、持久版本、未知报告响应与旧 Pending/新参数回调协调仍未交付；完整 TCC 和全计划继续开放。
