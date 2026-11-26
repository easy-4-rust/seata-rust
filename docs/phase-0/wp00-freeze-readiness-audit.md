# WP-00 冻结前核对 — 2026-11-12

规格事实源仍为[架构 Phase0–7 路线](../architecture/SEATA_RUST_ARCHITECTURE_PLAN.md)、[WP-00–07 验收清单](v1-implementation-acceptance-checklist.md)及已确认 ADR。本文只核对完成度，不修改退出条件、不另建规格体系，也不把独立探针改称正式组件。

| 原有门禁 | 已有实际证据 | 仍需完成 |
|---|---|---|
| 固定源码与协议映射 | Java/Go/RSeata 固定提交；29个常规样例、六个原有帧；新增心跳/合并线缆映射 | 全部必需消息和字段变体逐项对照，不能由样例推断全覆盖 |
| 候选样例的三方认证 | 原29样例认证；三个心跳三方字段/字节一致 | 三个失败响应差异与合并59/60缺少Go codec保持开放；Java生成的合并样例不能称三方 golden |
| Java V1 帧及真实 TC 接受 | 原六帧往返；真实 TM/RM 注册、空事务、模拟分支和二阶段回调 | 正式生产 Rust 模块的同样验证；任意消息/配置不能由这些路径推断 |
| 固定 Go/Java 脚本 | 原 Run42 与本地固定源码证据；本轮Java163、Go实际Getty诊断 | 更新后 wrapper 分别通过语法检查；尚无当前工作树的全新 Linux/CI 运行 |
| 枚举和状态语义 | 显式字段/码值断言；handler78个可编码结果实测 | GO-V1-STATE-001 的19号状态业务语义、fail-fast19/20分叉及其余未覆盖组合 |
| TC 状态与恢复 | FILE正常重启；DB/Redis八场景；RAFT三组真实集群、六次切换/三次快照重载 | FILE16启动失败/17过滤；RAFT16/17异步reload异常；自然超时及008–009真实Undo补偿/持久重放 |
| 调用链、资源适配器 | 固定源码调用链、handler及测试认证SPI分支；响应丢失不重发 | 正式 MySQL AT 资源适配器、数据库原子性和跨进程幂等 |
| RSeata 处置与复用 | 已有 no-copy/Keep-Refactor-Rewrite 文档 | ADR-0005/0006仍为 Proposed；若实际复用须单独许可与测试证据 |
| ADR 与公开契约 | ADR-0008已获用户确认：可选字段、明确拒绝/未知结果分离、TC负责重试 | 生产 API/配置及尚未确认 ADR 的相应门禁；已有确认不重复请求 |
| 任务与实施依赖 | 现有 Markdown WP-00–07 已分解 | OpenSpec未初始化；清单为OpenSpec-ready，不声称已执行其命令 |

当前独立 Rust 探针114项测试通过，Java选定163项、Python采集与来源校验5项通过；详见[控制帧证据](evidence/local-control-envelopes/README.md)。这些数量分别属于各自测试范围，不能相加为生产功能数量。Rust仍无正式 Cargo workspace，WP-01没有计为完成。

## 继续实施时的依赖约束

WP-01明确依赖WP-00；其验收又要求受支持消息匹配已认证Java/Go字节。因此不能通过改写原样例、给固定Go偷偷补codec或将已知不兼容称为通过来关闭WP-00。后续应先完成差异的修复/适配方案与验证、整理各消息认证状态，并对照清单确认冻结条件。

恢复调查与生产恢复验收也需区分：FILE/RAFT外部缺陷已有可重现证据，调查不是“尚未运行”，但安全恢复仍未通过。008–009包含真实资源补偿/持久幂等，需要正式RM/Undo能力支撑；如果现有顺序使WP-00等待后续WP的实现，应在正式计划中明确依赖及验收阶段。本文不自行取消或移动该门禁。可以继续准备规格、测试输入和实现方案，不据此提前勾选后续生产交付。

完整计划仍包含Phase1–7；当前未完成项和开放问题见[全计划进度](../delivery-progress.md)及[验证账本](validation-ledger.md)。

## 显式长度修复候选增量

GO-V1-FAIL-001/002现有独立源码副本的有界修复证据：原版21失败/三个成功控制；补丁24用例、80个codec/bytes顶层测试及原29/六帧回归通过；Java187/Rust115/Python6通过。原版Go未修改，上游问题不因此关闭。[证据](evidence/local-go-java-length-patch/README.md)。[分阶段冻结提案](wp00-staged-freeze-proposal.md)已准备供确认；批准前，本文上面的现行依赖约束保持有效。

## 异常码适配候选增量

0002显式响应入口现有25码×10类响应的250例证据：原版98差异、仅长度补丁60差异，显式入口全部匹配；原七个失败样例在该入口全部字段/字节通过。Java437/Rust116/Go三包104和新Python6通过。该候选保留原Go枚举/default行为，尚未接入传输；不能将原版或完整客户端互操作标绿。[证据](evidence/local-go-java-exception-profile/README.md)。分阶段提案尚待确认，正式WP-01仍按当前依赖约束处理。

## 合并载荷候选增量

38个新组合/边界样例与原六个合并样例通过显式0003载荷入口，覆盖25普通类型及250异常结果；Java475/Rust117/Go110和新Python7通过。源代码、执行测试、继承补丁及新鲜控制帧绑定均有完整审计。[证据](evidence/local-go-java-merge-profile/README.md)。原版59/60仍缺codec，实际Getty接入仍必需；不据此关闭完整GO-V1-MERGE-001或启动未批准的阶段调整。

## Getty 显式帧候选增量（2026-11-12）

0004在独立副本显式注入实际RpcPackageHandler，验证分片、连续帧、错误传播、有序可空头及长度边界。完整新鲜Java475/Rust117/Go181通过，新Python6和继承merge7/exception6/length6/control5通过。326帧中9帧由Java实际编码，317帧由工具封装Java认证载荷，不扩大Java帧认证数。

未修改的Java TC通过注册、Begin/Status/Begin合并请求、两个原始XID的Begin查询/回滚/Finished查询，8请求/8响应、零事务重发；端口和自有进程已清理。实际handler由自定义TCP探针调用，尚未验证dubbo-getty会话循环和原SDK的适配选择/完整业务分发。[证据](evidence/local-go-java-getty-profile/README.md)，[后续运行时缺口](go-getty-runtime-gap-audit.md)。原版/default兼容、生产Rust、RM/Undo和WP-00仍开放；分阶段冻结提案尚未获确认。

## SDK 会话与关联候选增量（2026-11-12）

0005显式配置接入原SDK InitGetty/SessionManager/dubbo-getty loop/listener/processor/future；新鲜Java475/Rust117/Go235（七包、222继承+13新增）和Python9通过，继承getty6/merge7/exception6/length6/control5另行通过，并发批响应重复20次通过。原掉响应、部分完成/越界、错误信封/类型、并发混合批结果及配置冲突均有失败/修复证据。

真实SDK17请求/17实际TC响应、两连接注册/心跳、11次重复并合并发送、5次晚到原响应和6种注入得到5次类型化未知结果；两个原XID回滚后Finished，零事务重发，受检future清理。原Getty EOF不自动重连已有独立失败与带owner/registry校验的候选修复，默认路径未自动启用。[证据](evidence/local-go-java-sdk-profile/README.md)。

新发现[JAVA-V1-PREFACE-001](java-v1-preface-fragmentation.md)保持OPEN：固定TC拒绝首个magic字节单独到达，实际负向连接/源码已归档。SDK场景只强制TC→SDK分片，不能据此声称任意双向分片兼容。原版本默认缺口、完整客户端初始化/自动聚合、在途故障恢复、RM/真实Undo、生产Rust及WP-00仍未完成；待确认的冻结提案不自动生效。


## Java / Go AT 共享锁实际缺陷（2026-11-13）

[计划](java-go-at-shared-lock-reference-plan.md)对应[070224负向主证据](evidence/local-java-go-at-shared-lock/README.md)。真实Java与Go AT在同一MySQL行同时保留已提交阶段一，余额1000→900→800、两个原Undo并存；固定Java锁键仅换resource即可看不到原锁，资源命名空间隔离由实际查询证明，锁键文本差异仍单独开放。GO-AT-RESOURCE-ID-001由源码推断升级为所选组合实际锁绕过，未认证安全互斥或迁移。

32 Python=归一化12＋实际证据破坏20；后者旧冻结校验器RED18失败/2控制、新门禁20通过。118运行/32追加输入、342classpath/2802依赖、16类来源、6原关键源/类、12原生查询72帧及真实TM/报告/原Undo分别闭合。Java客户端默认V2、Go V1原样记录，未扩张V1认证边界。原TM提交清理后余额800/Undo空/锁释放，3自有进程及容器退出、四端口释放。Go254/Rust117单元继承无新执行；正式Rust、WP00和全计划其余门禁仍未完成。


## 2026-11-13 — 恢复调查任务状态更新

[十项逐项核对](wp00-recovery-investigation-closure.md)与[只读审计](evidence/local-wp00-recovery-investigation-audit/README.md)完成现行任务“运行REC001–010调查或明确记录受阻项”：十一清单/20,880列出文件SHA匹配，原FILE/RAFT失败与未执行操作保持显式受阻，后续真实资源链路按各自范围引用。现行清单只勾选这一调查任务，安全恢复、候选完整验收、全部变体认证和整体WP00仍开放；WP01依赖和待确认提案不变。原上表记录的是2026-11-12时点，不以历史partial否认后续已有真实补偿，也不以调查完成声称生产恢复完成。


## 2026-11-13 — V2顺序明确批准并开始正式协议

用户明确回复“批准修订版及首期范围”，已应用V2和§7分阶段门禁，首期allowlist已接受，WP00-P已有逐项核对记录。正式Cargo workspace及seata-protocol的域枚举/码映射/字段原语已经实现并有[正式测试证据](../phase-1/evidence/protocol-foundation/README.md)。上文所有“尚未获确认/无正式workspace”表述只属当时状态。

当前只有P启动条件及WP01首三项已验证，整体WP00-R、恢复风险、完整互操作及后续生产Rust仍开放；不能把V2批准视为外部TC风险接受或完整客户端通过。
