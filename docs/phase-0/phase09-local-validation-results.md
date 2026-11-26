# Phase 0.9 本地验证增量

## 2026-11-12：真实三节点 RAFT 恢复诊断完成，异步异常保持开放

Begin1、CommitRetryTimeout16、RollbackRetryTimeout17三组原版TC集群，完成六次真实领导者切换与三次原节点快照重载。原XID/分支/资源/锁键与锁占用跨副本一致，控制台及原生V1共18次原事务查询可核对。正常Begin的有分支记录进入回滚重试5，空事务完成；16/17均保留可查询，但四次JFR精确捕获原SessionHolder.reload异步异常。记录保留和节点可用不等于恢复成功，登记JAVA-RAFT-RELOAD-001。

最终 `run_20261112_165835_c293c228` 诊断完成，Java新鲜4项、Rust106项、Python3项及静态检查通过。审查要求的异步等待、事件时间窗口、实际分支字段均已修正。完整JFR读取器解决默认五帧截断；失败尝试保留、17个大型JSON无损压缩并记录原始哈希。12个自有JVM退出、九个端口释放。[原始证据与快照](evidence/local-raft-retry-timeout/README.md)。REC003/004不再是未执行，但安全恢复验收、人工重试与真实补偿仍开放，全计划继续进行。

## 2026-11-12：认证钩子、拒绝通道与双资源重连通过

原版固定TC JAR分别运行默认策略／测试扩展开启／测试扩展关闭三个对照，均通过。开启扩展时16次注册拒绝均返回identified=false；同通道发送一次GlobalBegin后必须实际关闭，不能以超时充数，FILE全局数据仍为0。认证成功后，旧RM登记两个资源的原分支，新认证连接重登记完整资源列表；TC确认旧通道移除后，两个原BranchId分别完成提交和回滚回调、TC完成日志及Finished查询。没有重注册分支或重发结束请求。

最终 `run_20261112_161702_b4eda7e4` 成功，Java新鲜7项、Rust103项及静态检查通过；独立只读审查无新增问题。首次扩展优先级和第二次异步完成窗口的失败记录保留，均有先失败的回归测试。原TC默认策略仍允许所有请求，成功注册日志会包含extraData，登记JAVA-AUTH-EXTRADATA-001；固定公开凭据只是本地夹具，不能计作生产认证。[证据与复现](evidence/local-auth-registration/README.md)。REG003–006在此范围通过，Phase0和全计划继续进行。

## 2026-11-12：MySQL／Redis 八个后端重启与人工重试场景通过

每个后端分别写入16/17、有／无AT分支四类记录和真实锁；后端进程重启、独立读取JVM及原TC完整启动后，原XID、分支和锁均可核对。实际控制台人工重试触发原BranchId回调，模拟动作一次，目标记录消失、锁释放；空分支无RM动作。重复命令须匹配原XID及精确的缺失会话异常，不能拿任意HTTP500充数。观察事务的延迟4状态经显式TC管理重试完成，原目标不重发，最终DB三表归零、Redis全局/分支键清空。

最终 `run_20261112_152511_3907a673` 八场景通过，Java新鲜3项、Rust98项、Python5项通过，2122项生产源码／资源／POM及JAR／驱动／夹具类哈希已核对。审查发现的错误判定和构建溯源问题已修复；夹具端口0、观察事务状态、Jedis ABI和Spring配置桥接失败记录均保留。全部数据在D/E，自有容器停止、端口释放。

REC005–007为注入记录／模拟RM范围通过，008–009的真实补偿／持久重放仍未验证。最终原TC日志另有指标订阅者null endTime异常，登记JAVA-METRICS-RETRY-001，不能将事务结果当作指标通过。[完整证据和边界](evidence/local-backend-retry-timeout/README.md)。总体计划继续进行。

## 2026-11-12：FILE 重试超时记录的完整 TC 启动诊断

真实 Java FILE 管理器写入 Begin／AT 分支／状态更新，写入 JVM 退出后，新 JVM 读取同一磁盘数据，再启动原版固定 TC JAR。两次三场景运行均复现：Begin1 恢复原会话和锁；CommitRetryTimeout16 保留会话但启动抛 ShouldNeverHappenException；RollbackRetryTimeout17 过滤会话／分支，启动后原 XID 返回 Finished15，原锁可用。后者不是补偿成功证明。

初始 Java 联合集140/140，最终运行器每次重新编译并执行5项真实进程测试，修复审查发现的旧报告复用风险；Rust96/96、格式及静态检查通过。最终含空格目录运行已完成，端口释放、清理无错误。原版 Java 不作补丁，两个问题登记为外部恢复缺陷，REC-TIMEOUT-001–002 恢复验收保持开放。[原始记录及复现说明](evidence/local-file-retry-timeout/README.md)。其他存储、自然调度超时和操作员重试继续实施。

## 2026-11-12：INT-013–014 外部丢失响应验证完成

固定 Java TC 的真实 V1 通道前新增独立代理，每个场景只丢弃一个完整成功响应：BranchRegister12、Commit8、Rollback10。三者均没有重发原修改请求；原 XID 的回调、当前回调 RPC ID、一次模拟动作和锁释放均由流量及 TC 日志交叉核对。

丢失注册响应后客户端最初没有 BranchId，仅从匹配原 XID／resource／AT 类型和发送前已有非空 nonce 的真实回调学习身份；代理观察到的隐藏响应只由运行器在客户端结束后核对。丢失提交／回滚响应后，客户端后续只观察到 Finished15，因此按 ADR-0008 保留结果未知，不将锁释放或观察端成功记录转换为客户端成功。

Rust 新测试先实际出现两个目标失败；实现后总集93/93通过，代理先5项失败、实现后5/5通过，包含真实本地双通道转发／丢弃／退出检查。格式与静态检查通过，独立只读审查无发现。默认及含空格运行目录两次完整三场景均通过，最终进程清理无错误且端口已释放。

[完整证据与复现入口](evidence/local-phase09-response-loss/README.md)。INT-013–014 在一次、同连接、单个待定模拟分支范围通过；其他丢失／重连、真实认证、重试超时恢复及 Phase1–7 正式组件继续开放。下文较早增量中的未完成声明按各自历史运行理解。

## 2026-11-12：真实 Java 处理器78个响应的全量 Go/Rust 比较完成

重新运行固定 Java 目标135项，通过后从独立字段／手工字节断言后的机器事件采集全部79个具名上下文：78个可编码载荷、1个空状态报告不可编码诊断。采集器拒绝旧成功日志缺失的79项记录，也拒绝漏项、重复、错误字段／类型／字节和已有不同文件；6项自身测试通过。原29与补充7样例不改动。

每个真实载荷均送入 Rust 核对规范化线缆字段与字节重编码，新增所有前缀截断及尾部数据检查；完整测试83/83。固定、未修改的 Go 完成78个原生类型往返；对 Java 输出结果为62个覆盖字段／字节兼容，6个既有错误长度差异，1个既有错误码差异，9个不透明状态用例。后者数字19和字节可保留，但 Go 的命名 GlobalStatus 只到15；这是 GO-V1-STATE-001 语义证据缺口，不是新字节丢失。

只修改 D 盘独立副本的预期版本2.8→3.8，保持真实字节不变，Go/Rust 均实际失败；原样例未改，恢复原路径后通过。格式／静态检查与只读审查通过，跨平台字节属性和源码／输入／报告／载荷哈希已核对。

[完整证据](evidence/local-handler-runtime-20261112/README.md)。新增一键 Bash 入口可让 Java采集、Go、Rust 使用同一新目录；本 Windows 执行的是等价独立命令，入口只做语法与只读审查，未声称远程 CI 执行。该比较项已完成，Go差异／状态语义、空状态报告编码、真实鉴权、响应丢失及恢复等门禁继续开放。

## 2026-11-12：Java handler72／注册处理器7，联合集135通过

新增测试运行固定 Java 的真实 handle/process，只替换核心操作、会话、鉴权钩子和通道依赖。八类 handler 覆盖正常／事务异常／运行时异常／存储异常，回填覆盖会话不存在、查询抛错、状态 getter 抛错和回调已修改响应状态等分支；另有锁冲突／快速失败、成功 lockable=false 和空状态报告诊断。72 项通过；可序列化普通响应还与手工构造的完整 V1 字节比较。

TM/RM 鉴权钩子允许／拒绝／抛错以及被拒绝的关闭 RM 通道共7项通过。V1 只传 identified 与 version：Java 对象错误消息不会传出，新解码对象可能 ResultCode.Success 但 identified=false，测试明确以 identified 为准。这些是处理器单元证据，不能当作真实凭证或网络鉴权验收。

最初故意保持三个旧夹具缺失的异常注入路径，新断言实际检出“应失败却成功”；完成夹具后运行真实固定处理器通过，未修改上游生产代码。联合集目标135/135，0 failures/errors/skipped，91模块 BUILD SUCCESS。审查无可操作发现。

新记录 JAVA-V1-REPORT-NULL-001：报告失败后保留 null 状态，随后响应编码实际抛出 NullPointerException。这仍是开放问题，不算线缆互操作通过。[完整证据](evidence/local-java-handler-matrix-20261112/README.md)。新运行输出的全量 Go/Rust 核对、真实鉴权、响应丢失与其余计划继续实施；总体目标保持进行中。

## 2026-11-12：新增 7 个失败对照，复现 3 项 Go/Java 差异

重跑固定 Go 原有 29 载荷／6 帧通过，原 JSON 和二进制不变。新增六类此前缺少的事务失败响应，以及同名 fail-fast 错误码对照；Go 原生 7 例与 Java/Rust 各自独立字段／字节验证通过，其中仅 4 个新对照可跨语言互操作。

另外 3 项保持开放：Go 的 BranchReport／GlobalLockQuery 失败消息长度为 1 字节，Java 为 2 字节；Go fail-fast 码为 19，Java 为 20，Java 的 19 实际表示 NotRaftLeader。原生 Go/Java 全字节相等检查确实失败，失败日志和源码已保留。Java/Rust 不猜测方言、不重映射合法 Java 错误码；诊断测试检出差异不算兼容通过。

本轮实际结果：Rust 81/81；Java 新增 7/7，目标联合集 56/56、91 模块 BUILD SUCCESS；导出器 5/5 及格式／静态检查通过。新增 14 个 Java/Go 二进制对照及来源／输入／哈希清单。独立只读审查无可操作发现。

[差异矩阵](known-go-v1-failure-divergences.md)与[运行证据](evidence/local-failure-variants-20261112/README.md)。所有普通事务响应族现在至少有一个 Java 失败样例，但并非全部字段变体，也未解决固定 Go 的三项差异。Java handler 异常矩阵、响应丢失及完整计划其余阶段继续开放。

## 2026-11-12：Rust 29 个载荷样例及恶意输入边界通过

独立探针新增仅测试时编译的逐消息类型和真实 V1 编解码。29 个冻结样例均通过独立字段预期与逐字节重编码；四个失败响应保留错误消息、异常码及状态。完整 Rust 回归为 73 项通过（39 项新增、34 项已有），格式与 Clippy 检查退出 0。

每个样例的所有截断前缀、尾部数据、非法长度／UTF-8／枚举、带符号整数、多字节字符及大小边界均有测试；所有单字节变异须拒绝或完整重编码。D 盘隔离副本仅将合法 timeout 2000→2001 后，独立语义断言实际失败，原 JSON 和二进制样例未改动。

审查发现并修复两个原样例外的合法边界：报告缺省状态使用 `0xff`，以及注册响应省略版本时 Java 保留构造默认值。两者均保留失败回归证据；新增 3 项固定 Java 实测通过，Java 载荷目标集合计 32/32。Rust 分别保留线缆缺省信息和 Java 默认值适配，未把省略版本误写为 Java null。

更新后的 Java 联合集目标模块 49/49 通过（29 载荷 + 3 缺省字段 + 6 帧 + 8 异常 + 3 恢复），91 模块 BUILD SUCCESS。上游 serializer 模块的 32 项不重复计入 49。Maven 格式化后的实际执行源码及报告已归档，Java 词法单元与仓库覆盖测试一致，四份 JSON 资源与 canonical 原始字节一致。

证据：[local-rust-payloads-20261112](evidence/local-rust-payloads-20261112/README.md)。这关闭冻结 29 样例的 Rust 子门禁，不代表正式协议 crate、所有消息变体或 Phase 0 完成。后续仍需核对失败 BranchReport/GlobalLockQuery、merge/heartbeat、响应丢失及其余异常／恢复矩阵。

## 2026-11-12：29 个 Go payload 的 Java 交叉验证及二进制样例通过

新增 PayloadGoldenCorpusTest：读取未修改的 canonical expected.json，逐例验证类型、关键字段／失败状态与可选字段语义，再要求 Java V1 重编码与 Go 样例逐字节相同。25 类基础消息和 4 个失败响应共 29 样例全部通过；与原测试组合后目标模块 46 tests、0 failures/errors/skipped，91 模块 BUILD SUCCESS。

测试保留 TM ExtraData 空字符串与 RM null 的差别。反向验证仅修改 D 盘临时副本的 timeout 2000→2001，成功检出字段预期不符；随后发现 Maven 保留了较新的临时编译资源，通过原始哈希定位并恢复源码／构建资源后重跑通过。E 盘 canonical 样例从未修改，Git blob 与基线相同。

29 个原始序列化二进制样例已放在原计划指定的 `tests/fixtures/protocol/v1/`，manifest 包含版本、编解码信息、输入来源及每文件哈希。导出器在已记录 RED 后实现，六项测试覆盖拒绝覆盖、非法前缀／路径、大小写碰撞和 CRLF/LF 一致性。独立复核问题已修复。

准确源码、JUnit 报告、命令、失败历史与哈希：[local-java-payloads-20261112](evidence/local-java-payloads-20261112/README.md)。这些结果证明当前样例的 Java/Go 兼容，尚未完成 Rust 全语料或恶意输入验证；整体 Phase 0–7 目标继续进行。未创建分支、提交、推送或更改远程工作流。

## 2026-11-12：当前精确 Java 联合覆盖测试验证通过

复核现场发现此前另有本地 17/17 联合证据，原 Oracle 覆盖文件与当前仓库版本仅有格式／断言失败诊断差异。保留所有旧文件后，从固定 Java commit 导出新的 D 盘源码副本，复制当前三个覆盖文件重新执行；运行后逐个核对哈希，全部与当前仓库文件一致。

结果：91 个 Reactor 模块 BUILD SUCCESS，目标联合模块 17 tests、0 failures/errors/skipped，耗时 5 分 5 秒。这里只执行了指定测试集，不是全部上游测试。另通过 GitHub 只读工具直接确认已有 Run #42 的 Java／Go jobs 均 completed/success，没有触发工作流或推送变更。

当前本地联合验收／覆盖文件溯源缺口已关闭；不声称查明所有历史失败原因或当前 HEAD 远程 CI 通过。精确命令、测试文件副本、Surefire 报告、哈希和原始日志位置：[local-java-exact-20261112](evidence/local-java-exact-20261112/README.md)。

## 2026-11-12：正常 FILE 会话/分支的强制进程重启通过

使用固定源码对应的已验证 JAR，分别验证 Begin-only 和带 AT 分支两种正常状态。准备客户端记录 TC 颁发身份后退出；测试强制结束 TC，保存磁盘日志摘要，再以同一配置、端口和数据目录启动新 TC，使用全新客户端重新注册。

最终重跑（含空格数据目录）结果：

- Begin-only 原 XID `198.18.0.1:28091:4045076318255378433`，TC PID 71992 → 25876；重启后 Begin=1，回滚=11，最终 Finished=15。
- AT 原 XID `198.18.0.1:28091:4045076318342127617`，原 BranchId `4045076318342127618`，TC PID 60716 → 69304；新客户端收到原分支回滚回调，服务端确认处理成功。没有重建目标事务或重新注册目标分支。
- 使用不同的 contender XID 查询同一锁，结果 false（重启前）→ false（重启后）→ true（回滚清理后），验证锁恢复与释放；探测用的空事务单独结束。
- 两次运行都使用全新目录，不清理或重用旧数据。磁盘 root.data 在强制退出后、新进程启动前记录长度/哈希；最终两场景分别为 129／774 字节。
- 34 项 Rust 测试、fmt、Clippy 通过。复核发现路径含空格时 JVM 参数被拆分，已实际复现、修正引用并完整重跑；失败运行保留。

证据与时序图：[local-phase09-restart](evidence/local-phase09-restart/README.md)。INT-011–012 在正常 FILE 记录、进程强制退出/重启范围内通过，不外推为断电、文件损坏、重试超时状态或其他存储保证。整体交付仍以 [全计划跟踪](../delivery-progress.md) 为准。

## 2026-11-12：首次 RM 应答丢失与 TC 重投通过

新增 `--callback-loss`：在同一 RM 连接上，模拟动作成功后故意不发送首次二阶段应答，随后等待真实 TC 超时并重投。独立配置将回调超时与恢复轮询设为 1000 ms，不修改固定 Java 源码或普通测试配置。

| 场景 | XID | BranchId | 首次 / 重投 RPC ID | 模拟成功动作次数 | 初始 TM 响应 / 后续查询 |
|---|---|---|---|---|---|
| 提交 | `198.18.0.1:28091:4045076258168659969` | `4045076258168659970` | 1 / 2 | 1 | Success，Committed=9 / Finished=15 |
| 回滚 | `198.18.0.1:28091:4045076258168659971` | `4045076258168659972` | 3 / 4 | 1 | **Failed**，RollbackRetrying=5 / Finished=15 |

回滚的初始失败没有被改写成成功。第一次真实运行发现该响应后停止，原始失败日志保留；依据固定 Java 的异常状态回填与重试实现，补上失败回归测试，再增加仅此故障场景可继续观察的入口。普通流程仍拒绝失败响应。没有重发 GlobalCommit、GlobalRollback 或 BranchRegister。

进程内回调状态每次都校验完整分支身份；重复投递返回原分支终态，并关联本次 RPC ID，不再次应用成功的模拟动作。该模型不涉及数据库/Undo Log，也不保证失败动作无部分副作用或跨进程去重。

最终 29 项 Rust 测试、fmt、Clippy 均通过。真实 TC 重跑成功，服务端日志确认最终两类分支响应与成功处理，测试进程已停止、端口释放。本轮复用已有固定 JAR，没有重新构建 Java，也没有提交、推送或修改工作流。

证据：[local-phase09-callback-loss](evidence/local-phase09-callback-loss/README.md)。INT-010 仅部分通过：一次丢失后的同连接重投。INT-011–014、任意重复流、重新连接、认证、真实 AT 补偿与 Java 完整联合测试差异仍开放。下一步应单独验证 FILE 重启后的事务与分支恢复。

## 2026-11-12：BranchRegister 与真实 RM 回调通过

独立探针新增 `--branches`，沿用固定 Java SHA 和上轮成功构建的 JAR。两条已注册连接分别承担 TM 请求和 RM 接收；一次测试仅接收预期回调，不提供长期 RM、心跳或重连服务。

- 两笔事务分别取得不同 XID 和正数 BranchId，分支类型 AT，资源 `mysql://orders`，锁 `orders:1`，applicationData 为 `{}`。
- Commit 场景：XID `198.18.0.1:28091:4045076255862636545`，BranchId `4045076255862636546`。RM 收到 BranchCommit，回显 requestId=1，返回 PhaseTwo_Committed=5；TM 收到 Committed=9，查询 Finished=15。
- Rollback 场景：XID `198.18.0.1:28091:4045076255862636547`，BranchId `4045076255862636548`。RM 收到 BranchRollback，回显 requestId=2，返回 PhaseTwo_Rollbacked=8；TM 收到 Rollbacked=11，查询 Finished=15。
- TC 服务端日志独立记录两类分支应答及分支成功处理，避免将 AT 异步提交给 TM 的提前响应当作回调完成证据。
- RED/GREEN：先复现回调/BranchId 缺失的 3 项失败，再复现双向 TCP 生命周期缺失；独立复核发现重复 XID 校验缺口，新增测试实际暴露第二次分支注册，再补拒绝逻辑。最终 21 项 Rust 测试通过，fmt、Clippy 均 exit 0。
- 复核修改后重新执行真实 TC 测试通过，首次运行记录另存 `initial-live/`。测试进程已停止，28091 端口已释放；Windows 强制停止不证明优雅停机或恢复正确。

原始证据、时序图和复跑方式：[local-phase09-branches](evidence/local-phase09-branches/README.md)。本轮没有提交、推送、切换分支或修改远程工作流。

规格状态：INT-004、INT-006–009 在模拟 AT 资源范围内通过，Phase 0 保持开放。后续优先验证重复回调/响应丢失与 FILE 重启恢复；数据库 AT 补偿、Undo Log、认证、长期 RM 行为及 Java 完整联合测试差异仍未完成。

## 2026-11-11 续跑结果：真实 TC 空事务通过

本节为续跑结果，下文保留上一次构建超时的历史记录。

- 固定 Java SHA 未变，使用 JDK 17.0.12；临时 Maven 配置仅指定 Maven Central，未修改全局配置或项目 POM。
- 在线默认配置的构建等待远端 HTTP 校验响应；离线构建明确暴露服务端依赖未缓存。使用临时中央仓库配置后，89 个模块 `BUILD SUCCESS`，耗时 4 分 50 秒，生成 `server/target/seata-server.jar`（186546352 字节）。
- 首次 Windows 启动因 Seata 参数解析器拒绝 `--spring.config.location` 退出；将配置位置改为 JVM 系统属性后启动成功，失败日志保留。
- Java TC：FILE 存储、认证关闭、端口 28091；运行数据在 `D:\codex-builds\seata-phase09-live-20261111`。
- Rust TM、RM 均收到 `identified=true`，服务端版本 `2.8.0-SNAPSHOT`。
- 提交 XID：`198.18.0.1:28091:4045075949638950913`；查询 Begin=1，提交响应 Committed=9，后续查询 Finished=15。
- 回滚 XID：`198.18.0.1:28091:4045075949638950914`；查询 Begin=1，回滚响应 Rollbacked=11，后续查询 Finished=15。
- 两个 XID 的请求与状态均可在 TC 服务端日志交叉核验，探针退出码 0。
- 本轮 Rust 测试 13 passed，格式和 Clippy 检查退出码 0。没有修改已有 Rust 实现。
- Windows 测试进程通过 Stop-Process 强制停止，端口已释放；不据此宣称优雅停机或持久化恢复通过。

原始客户端/服务端日志、构建摘要、产物/配置摘要和可复跑脚本位于 [evidence/local-phase09-live](evidence/local-phase09-live/README.md)。本轮没有提交、推送或修改远程工作流。

当前状态：独立 Rust 空事务生命周期已现场验证；Java 完整联合测试差异、BranchRegister、RM 二阶段回调、AT 和 FILE 重启恢复仍待完成。Phase 0 保持开放，下一步为 BranchRegister 与双向 RM 回调验证。

## 基线和执行位置

- Seata-Rust：`130ad634279cef10cd2998fc3bfae82996224d66`，加本轮未提交改动。
- Java Oracle：`cba73e180fd8896a99df6415b859d275122feaa7`。
- 隔离工作区：`E:\workspaces\workspace-github-easy-4-rust\seata-rust-phase09-validation`。
- Java 隔离源码：同级 `seata-java-phase09-oracle`。
- 原 `seata-rust` 工作目录的未跟踪 `docs/` 未修改。本轮没有 commit、push、merge 或远程工作流变更。

## 已落地的测试工具变更

独立探针实际路径是 `tools/java-tc-integration/rust-client/`，不是部分旧文档写的 `tools/rust-registration-client/`。

新增 `src/transaction.rs`，通过 `--lifecycle` 执行两个不同 XID 的空事务：

1. GlobalBegin → GlobalStatus(Begin) → GlobalCommit → GlobalStatus。
2. GlobalBegin → GlobalStatus(Begin) → GlobalRollback → GlobalStatus。

读取响应时核验协议版本、帧长度、消息种类、序列化/压缩标识、Request ID、消息类型和剩余字节。保留失败响应中的错误码、消息和 GlobalStatus。只有收到对应操作的成功响应后，才允许后续 Status=Finished 表示会话已移除；Finished 本身不证明提交成功。

请求写入或读取响应失败时保守报告 outcome unknown，不自动重发 Begin、Commit 或 Rollback。探针记录已经收到的 TC-issued XID。

## 本轮 RED/GREEN

### Rust 生命周期

先增加测试和未实现入口，实际执行出现 6 项失败；补齐实现后通过。包括正确响应、截断、错误 ID/消息种类、不支持的帧、失败状态保留、Finished 与 Committed 区别、两条空事务链以及响应丢失不重发。

### TM/RM 客户端版本不一致

用本地 TCP 测试端捕获两种注册请求，实际复现：TM 使用 `1.1.0`，RM 使用 `V1,0`。将 RM 运行路径改为共同的 `CLIENT_VERSION` 后回归测试通过。此测试证明请求构造一致，不等于真实 TC 认证通过；帧中的协议版本字节仍为 1。

Go/Java 的 `V1,0` 静态字节 Fixture 没有修改。测试样本字符串与运行中的客户端版本声明是不同层次。

## 已执行的验证

| 项目 | 命令/范围 | 结果 |
|---|---|---|
| Rust 基线 | 原探针 `cargo test` | 2 passed |
| Rust 扩展后 | `cargo test --manifest-path tools/java-tc-integration/rust-client/Cargo.toml` | 13 passed，0 failed |
| Rust lint | 同 manifest，`cargo clippy --all-targets -- -D warnings` | exit 0 |
| Rust 格式 | 同 manifest，`cargo fmt -- --check` | exit 0 |
| Java 独立协议模块 | `mvn -pl serializer/seata-serializer-seata -am -Dtest=ProtocolV1GoldenFrameTest -Dsurefire.failIfNoSpecifiedTests=false test` | 6 passed，0 failed，0 errors，0 skipped |

原始 Rust 日志、Java Surefire 摘要和改动文件 SHA-256 在 [evidence/local-phase09](evidence/local-phase09/)。

Java 全量 `test-suite/test-new-version` 本地首次构建在进入目标测试前超过 600 秒，不能记作协议测试失败或通过。此前完整 CI 曾报告两个注册帧的重编码长度不一致；本轮固定源码的独立协议模块没有复现。因此本轮仅增强失败诊断，保留原始字节相等断言，没有修改 Fixture 或放宽验收，也没有宣称完整 CI 已修复。

## 现场 TC 联调

本轮尝试了固定 Java 源码的真实服务端打包：

```text
mvn.cmd -B -ntp -l phase09-server-build.log -Prelease-seata-jar -pl server -am -Dmaven.test.skip=true -DskipTests -Dcheckstyle.skip=true -Dspotless.skip=true package
```

该命令在 600 秒执行限额处超时；最终日志停留于 `seata-all` shaded artifact 和后续依赖收集阶段，未取得完整 Reactor `BUILD SUCCESS`，`server/target` 中也未发现服务端 JAR。构建超时不是事务正确性失败，亦不能记作构建通过。

因此本轮没有启动本地 Java TC，也没有对真实 TC 执行新增 `--lifecycle` 场景。结束时检查未发现该 Oracle 的遗留 Java 进程，测试端口 28091 无监听。下一次应先完成打包/启动，再采集现场 XID、响应状态和停止结果。

完整构建日志保留在 Java 隔离工作区的 `phase09-server-build.log`；尾部和执行状态另存入本项目 evidence 目录。

## 变更复核

已复核请求 ID、响应类型、长度边界、状态解析、失败后不重发、默认注册模式不变及无业务数据库操作等边界；`git diff --check` 通过。本轮没有独立第二审阅者，提交前仍需审查完整 diff。

## 不在当前证明范围

BranchRegister、RM 二阶段回调、MySQL AT、本地业务数据/Undo Log 原子性、认证、持久化重启恢复、完整 17 项 Java 联合测试、生产就绪均未由以上结果证明。Phase 0 不因此自动关闭。
