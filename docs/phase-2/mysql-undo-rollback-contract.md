# 原Normal Undo的真实补偿

细化既有WP05/WP06与[围栏契约](mysql-undo-fence-contract.md)，不关闭原十四项、不接受外部TC风险，共享锁仍暂缓。原来源固定为 `cba73e180fd8896a99df6415b859d275122feaa7` 的 AbstractUndoLogManager、AbstractUndoExecutor、DataCompareUtils 与三个 MySQLUndo*Executor。

1. 独立、有限、拥有实际连接的控制事务按原XID/BranchId锁定原Undo唯一身份。缺失/已有完整GlobalFinished沿既有围栏语义处理；Normal日志保留原身份，验证原Jackson/NONE上下文、全部分片与长度预算后再读字节。未知压缩、状态、子行身份、缺片、重复子号或超限保持错误，不删除日志。

   缺少 Normal 根行时，回滚在原身份锁内用当前读检查同一 XID 和原父号的子片。发现残留子片即返回提交前错误，并回滚本次尚未提交的新围栏。已有 GlobalFinished、子片和业务数据均保留。无关联子片时继续既有空回滚路径。托管 RM 保留已确认的回滚方向和非终态，由同一分支后续恢复继续。

   固定 MySQL 8.4.13 现场先复现缺根和已有围栏两条路径都错误返回 `Fenced`。修复后的用例核对原 Undo 七列、业务值和无关联子片时的幂等围栏。托管 RM 用例确认有孤儿片时返回 Unknown，并在夹具清除残缺子片后用同一身份完成空回滚。 [修复前](evidence/mysql-undo-rollback/run-orphan-a14064e8ffb2/1.99.0/test.log) · [修复后](evidence/mysql-undo-rollback/run-orphan-f7adda0bb0ba/)。
2. 原文档与本事务持MDL取得的物理元数据绑定，核对原身份、完整列/PK标记、JDBC配置/类型和值域，再形成SqlUndoLog。数据库副作用前验证全部日志；不拿宽松JSON文档校验当作可执行认证。首期仍为已有完整镜像和原生类型矩阵；原Java其他类型、投影和数据库仍在完整迁移计划中。
3. 逆序处理SQL日志。before==after跳过；锁定当前完整主键元组，current==after才按原执行器恢复，current==before跳过，否则报脱敏脏数据错误并整体回滚。复合主键使用完整元组、原生值与绑定参数，禁止字符串拼接成身份或笛卡尔乘积。UPDATE只恢复非PK，DELETE补偿插入全部字段，INSERT补偿按PK删除；每行更新计数和恢复后镜像均核对。
4. 所有补偿与原root/子日志删除共用一个本地事务。任何类型/脏数据/驱动/取消/期限错误不发布部分成功；控制COMMIT可能发送后无权威回复统一为提交未知，保留原身份、不自动重发。最终Ready仍复核绝对期限。显式后续恢复使用同一原身份；不能自动执行原业务SQL。
5. 原Normal路径成功后删除原日志；本步保持这个源语义，不假造原来源为“转换成GlobalFinished”。`Consumed`只描述本次日志处理和控制提交，不作为所有旧尝试已被阻断或通用RM Applied/NotApplied的证明。终态结果记录、正式TC所有权、所有旧尝试围栏与资源适配器仍须单独完成；缺失日志路径才持久写入GlobalFinished。
6. TDD覆盖真实INSERT/UPDATE/DELETE、多行复合PK、逆序混合日志、binary/NULL/精确数字、当前已恢复/无变化、脏数据整体回滚、schema/类型/身份和分片破坏、取消/期限、并发重复以及实际提交故障。原Java执行器/校验/原表存储方法分别使用未修改来源对照并保留准确范围，不声称原JUnit或完整manager已迁移。

业务公开COMMIT、TC安全门禁、生产TLS/多节点/GC、原 TC 锁覆盖、WP05/WP06、完整 V1 及全部后续 Phase 保持 OPEN。真实 MySQL 已验证四种二进制排序规则下的文本镜像和 Undo 恢复。 [验证记录](wp05-mysql-bin-text-images.json)

原默认JDBC将某些历史TINYINT(1)值2编码为true，原信息已经丢失；本组件不承诺这类旧Java日志可逆，也不能把新生产器拒绝有损编码视为历史数据已获认证。正式资源接入仍须识别生产器/配置与来源边界。

## 完整镜像的主键匹配

回滚镜像比较按原生主键元组逐个匹配，每个主键只可匹配一次。两侧重复主键均返回不等。合法的不同行顺序仍可匹配，且 `("a_b","c")` 与 `("a","b_c")` 始终是不同元组。构建 `SqlUndoLog` 时已经拒绝重复身份。比较层的检查为接收完整镜像的路径提供独立防护。

固定原 Java 的两个 `DataCompareUtils` 测试类共 12 个实际用例通过。追加 Java 探针显示左侧重复和右侧重复的结果不同，字符串拼接还会将上述两组元组压成一个映射键。Rust 保留回滚契约的原生元组与对称匹配。原 Java 的可空对象、Fastjson 类型转换及映射层测试继续按逐案迁移契约验证。 [比较器验证记录](wp05-mysql-record-comparison.json)

## Fastjson 原 Undo 解码与元数据绑定

按固定 Java FastjsonUndoLogParser 的原载荷验证完整分支、SQL 日志和前后镜像。64 位整数标记、十六进制二进制、BigDecimal 尾点及省略的空字段进入有界物理类型绑定。禁止重复成员、非白名单类型、对象引用及不明词法。身份、完整有序列与主键、方向和精度沿用既有绑定规则。真实数据库回滚启用仍要求独立现场验收。

固定 Java Fastjson 生成并重新解码同一原载荷。两套 Rust 对照三个 SQL 日志、六份前后镜像和 36 个物理字段值。当前范围为完整、无对象引用的既有 MySQL 物理类型。Fastjson 原测试类的七个具体案例另有逐案迁移证据。 [Fastjson Undo 解码验收](wp05-fastjson-undo-native.json) [原类测试覆盖](wp05-fory-undo-source.json)

Fastjson 恢复前的文档解析核对调用方提供的分支身份、完整 SQL 日志结构、表标识符、镜像表名与行方向。预检拒绝缺失的必需键、空主键及大小写冲突的列名。按首现顺序返回去重表名供同一控制事务查询。物理绑定使用同一经过验证的文档，重新核对身份、原始和归一化字节预算及当前元数据，所有日志绑定之后才可能进入补偿执行。

表范围预检与完整物理绑定已经过固定 Java 载荷和双 Rust 回归验证。必需字段缺失返回解码错误。跨解析器绑定重新检查原始和归一化长度。真实 MySQL 控制事务接线与现场验收仍为后续门槛。 [预检验收](wp05-fastjson-undo-preflight.json)

Fastjson 控制事务验收路径要求锁定原 Undo 头部声明的编码与本次调用指定的解析器精确一致。分片读取、表元数据读取、全量绑定、反序补偿、删除与提交未知分类沿用现有控制事务。在真实 MySQL 验收完成前，常规回滚入口及提交见证继续要求 Jackson。Fastjson 路径由仓库内部真实 MySQL 测试调用。

内部验收入口已接入精确序列化器分派。真实测试构造一个根行和一个子片，先核对默认入口拒绝 Fastjson，再验证错误的完整镜像保留业务与 Undo，并以服务器 DML 计数核对补偿前全量绑定。修正镜像后的目标结果为反序执行三项补偿并清理两条 Undo。测试已编译，现场执行依赖固定 MySQL 环境。 [控制事务接线证据](wp05-fastjson-undo-control-transaction.json)

## Fastjson2 JSONB 的离线物理绑定

固定 Java Fastjson2 2.0.52 原解析器生成两份 JSONB 分支并重新解码，确认装箱类和物理值。每份载荷包含 INSERT、UPDATE 和 DELETE 三个 SQL 日志、六份镜像及 36 个字段值，覆盖 Long、Integer、无符号 BIGINT 的 BigInteger、保留尺度的 BigDecimal、Unicode、二进制与空值。增加了 DECIMAL(65,30) 和两种 BigInteger 编码的验证。

Rust 直接把 JSONB 装箱值绑定到调用方提供的物理元数据。绑定前核对分支身份、表标识符、镜像表名、日志方向和完整主键标记；绑定时按实际列序核对 JDBC 编号、装箱类、可空性及物理值域。编码预算按原 JSONB 字节计算，紧凑数值计入标量预算。序列化数组只用于原对象对照。现场 MySQL 回滚、生产分派和 WP05 验收尚待完成。 [Fastjson2 物理绑定证据](wp05-fastjson2-undo-physical.json)

内部验收入口按锁定根行的 `fastjson2/NONE` 上下文选择 JSONB 解析器。已预检的只读文档按首现顺序提供唯一表名，并在同一控制事务内绑定这些表的物理元数据。真实测试用原 Java 载荷构造根行和子片，要求默认回滚拒绝该格式、损坏列名时保持 DML 计数与原日志、修复后反序补偿三条 SQL 并清理两片。双 Rust 工具链已编译真实测试。Docker Engine 返回 HTTP 500，现场测试和生产分派尚待验收。 [控制事务接线证据](wp05-fastjson2-undo-control-transaction.json)

## 控制事务失败与清理

独立 Undo 回滚入口和托管 RM 补偿控制事务在原期限内且控制 COMMIT 之前收到具体错误时，先保留该错误，再在原绝对期限内尝试显式 ROLLBACK。托管 RM 仍依既有契约将内部错误映射为分支结果，由 TC 决定后续重试。清理失败或未在期限内完成时，仍返回原错误和事务身份。连接在进入事务前标记为关闭，因此错误或取消后由 SQLx 关闭连接。若尚未读到具体错误，超时返回期限错误。可能发送 COMMIT 后超期仍属于结果未知，调用方只能沿原身份恢复。

固定 Java UndoLogManagerBehaviorTest 的 15 个原用例均已执行。Rust 离线故障测试验证脏数据、已观察的读取错误、连接获取错误及迟到的 COMMIT 完成分类。Java 的 JDBC 资源及事务设置断言与 Rust 的真实 SQLx 现场验证分别记录。 [控制事务失败验证](wp05-undo-rollback-failure.json)

回环协议夹具使用实际 SQLx 0.8.6 连接和生产回滚入口。对端记录首条连接的 BEGIN、SHOW GRANTS 与 ROLLBACK，先让元数据读取返回 SQLSTATE 42000、错误码 1142，再分别返回回滚错误和扣留回滚回复。测试核对首错和原事务身份、未发送 COMMIT，以及容量为 1 的池随后建立第二条物理连接。夹具覆盖 SQLx 线缆和本地连接池行为，真实 MySQL 的事务与数据状态仍由独立现场验收证明。 [SQLx 回环验证](wp05-undo-rollback-sqlx-cleanup.json)

托管 RM 补偿控制事务复用该回环夹具验证元数据失败后的清理。期限内观察的错误跨 ROLLBACK 错误或超时保留原分支身份、SQLSTATE 和错误码。期限后首次观察的失败继续使用原期限分类。回环覆盖控制事务及连接池，完整 TC 回调和真实 InnoDB 仍由现场验收验证。 [托管回滚 SQLx 证据](wp05-managed-rollback-sqlx-cleanup.json)

SQLx 在语句准备阶段收到 MySQL 拒绝时，返回原 SQLSTATE 和编号。驱动在准备期间断开时，错误链保留 I/O 类别和可用的操作系统编号，驱动消息和参数不进入公开错误。按日期清理须符合围栏生命周期契约。固定 Java 驱动异常用例的 SQLx 对照目前覆盖写入和单子片读取 PREPARE，仍有按日期删除及异常对象身份差异。 [围栏生命周期契约](mysql-undo-fence-contract.md) · [驱动诊断验证](wp05-undo-driver-prepare.json)

独立 Undo 回滚在获取连接时保留 MySQL 拒绝的原 SQLSTATE 与错误码。若 SQLx 连接池在控制操作总期限前耗尽获取预算，报告为连接获取阶段期限错误。总期限先到时，报告为本次回滚的期限错误。未取得连接时保持控制提交前失败并保留原分支身份。SQLx 回环覆盖认证拒绝和两种期限顺序。托管控制事务使用相同的驱动分类。若真实 RM 回调因 MySQL 连接获取失败而无法持久确认来源，回包保留原身份并返回可重试回滚状态，文件日志保持 Pending，由 TC 发起后续请求。回环覆盖 Rust RM 的实际回调处理和文件重放，数据库连接失败位于回调来源确认阶段。 [连接获取验证](wp05-undo-acquisition-sqlx.json) · [托管回调验证](wp05-managed-acquisition-callback.json)

## 补偿事务的当前约束检查

独立及托管 Undo 补偿在读取业务表字典前，必须在原控制事务持有目标表 MDL 时，使用原数据源认证配置的新会话检查当前触发器、入向外键及出向外键，并核对 server UUID 和 schema。所有日志表均通过检查后才允许第一条补偿 DML。此检查受原控制操作绝对期限覆盖，每次检查临时增加一个池外连接。约束拒绝、连接错误、取消及超期继续沿用已有控制事务回滚与提交前失败分类，不清除原 Undo，不合成分支终态。稳定单主路由要求与本地业务入口一致。

固定 MySQL 8.4.13 已复现旧实现遗漏迟到触发器并提交其副作用。修复后，业务表 MDL 等待期间提交的触发器及两类外键均被当前约束检查拒绝，原身份、Undo 七列、业务和子表行保持不变。Rust 1.99、1.85 各通过 16 项独立 Undo 及 11 项托管恢复现场回归、633 项工作区测试和严格 Clippy。迟到约束的精确锁调度通过独立入口执行，托管接线经源码审查并运行既有完整现场套件。 [当前约束验证](wp05-undo-current-constraints.json)

## 原 Undo 分片写入预检

写入入口按会话 max_allowed_packet 的 80% 规划片长，先验证总字节数及根上下文最小容量，再分配并校验完整子片身份。子片按生成顺序写入，使用原根号作为父引用。根片最后写入，记录同顺序的 `subId` 引用及明确的 `compressorType=NONE`。每条 INSERT 绑定原 XID，整个写入由调用方的同一事务控制。读取和写入共用 32 MiB 字节上限。

固定 Java 原类三例均已运行。分片写入用例的字节、顺序、身份和上下文已对照。回环对端进一步逐行核对 SQLx 的 XID、Normal 状态、分片参数、EXECUTE 和 COM_STMT_CLOSE。写入使用一次性预编译语句，发送 EXECUTE 后即发送 CLOSE。第二子片返回 1062 时终止后续写入，调用方执行 ROLLBACK。真实 MySQL 分片存储和事务原子性继续沿用现场验收。 [分片布局验证](wp05-undo-fragment-layout.json) [SQLx 分片写入验证](wp05-undo-fragment-sqlx.json)

当根行的 `subId` 值按固定 JDK 的 `Character.isWhitespace(char)` 为空白时，视为未声明子片，仍执行原父号子片成员查询。非空白的值继续要求规范且互异的正整数子号，读取兼容有或无单个尾分隔符，写入保留固定 Java 的尾分隔符。不可解析的子号和超出 i64 的数值通过已有回滚错误链返回数值解析原因，错误不记录子号正文。根片正文和子片批量正文查询分别完整消耗结果并关闭临时预编译语句。子片查询按声明次序绑定子号并在末尾绑定原 XID，控制事务仍由原调用方处理。读取路径核对根行声明与实际子片结果集等长，并确认每个预期子号都出现。SQLx 回环证据显示，旧检查会让重复子号掩盖缺片并发出正文查询。修正后，重复、缺失或孤儿片在发出正文 SELECT 前被拒绝。成员结果即使乱序，正文仍按根片在先及 subId 声明次序重组。SQLx 回环对端记录正文语句的 COM_STMT_CLOSE，并扣留最后一个结果集结束帧验证读取仍在等待。无效数字的解析原因可通过公开的 `DbError::InvalidUndoSubId` 检查。固定 Java 读取用例的 `101,102`、原 XID、子片长度与有序载荷已在生产读取路径逐项对照。子片批量读取仍受 64 片和 32 MiB 总载荷预算约束，真实 InnoDB 锁行为待验收。 [分片读取回环验证](wp05-undo-fragment-read-sqlx.json) · [空白 subId 验证](wp05-undo-blank-subid.json) · [数值错误和资源验证](wp05-undo-read-resources.json) · [固定 Java 读取用例](wp05-undo-fragment-read-source.json)

Normal Undo 的清理按子片在先、根行在后执行。每条删除按原唯一键定位，同时逐字节验证 XID。子片删除还逐字节验证原父号上下文。任一删除返回零行、超过一行或驱动错误时，停止处理后续行，由调用方回滚同一事务。SQLx 回环证据核对了实际 DELETE 和全部绑定，确认第二子片的零行或错误响应后未发送根行 DELETE 或 COMMIT。

新采集的现场验证按明确标记校验字节级删除 SQL 与两次原 XID 绑定。历史线缆记录使用各自的原格式回放。验证器回放了三套检查器的九份历史记录，并用独立构造的数据核验新格式和负控。真实 InnoDB 的匹配、锁和回滚效果仍待现场验证。 [分片清理证据](wp05-undo-fragment-cleanup-sqlx.json)

## 补偿连接获取失败的真实 InnoDB 验证

固定 MySQL 8.4.13 镜像经摘要校验后在 D 盘 Ubuntu WSL 的隔离 Docker 引擎运行。服务端为 InnoDB、REPEATABLE READ，`lower_case_table_names=0`。Rust RM 接收受控 TCP 回调并持久确认来源和回滚方向。代理在第四次物理连接获取时拒绝认证，该连接对应补偿控制事务。首次回包为可重试状态 9，恢复行保持回滚方向和非终态，文件分支仍为 Pending，原业务行与 Normal Undo 七列保持原样。同一身份的第二次显式回调进入补偿，回包为状态 8，业务行和 Undo 清除，文件分支进入 Completed。故障移至来源确认阶段的负控触发预期回包断言。 [真实数据库证据](wp05-owned-wsl-mysql-validation.json)

Rust 1.99 和 1.85 各通过完整的 13 项回滚、10 项写入以及该补偿连接故障用例。TC 对端由测试控制，完整 Java TC 链路与原 WP05 门槛继续按计划验收。

## 固定 Java TC 的补偿重试

### 复合主键的混合事务

固定 Java TC 使用 MySQL 8.4.13 存储事务和锁。正式 Rust RM 经实验性一阶段接口在同一分支提交 UPDATE、DELETE 和两次 INSERT。三个操作先后作用于同一复合主键，其中一列超出 JavaScript 的精确整数范围。另一个 INSERT 使用不同的复合主键，两个相邻行提供隔离对照，其中一行保留 NULL。

原始报文核对两个完整锁键、原 XID、分支号以及唯一一次一阶段报告。Commit 在同一事务锁定原 Undo、删除日志并写入提交终态，四条业务行保持确认时的值。Rollback 在原物理事务锁定 Undo 根行，按原生整数参数读取两个完整主键的当前镜像，先删除两条后插入的行，再重建 DELETE 前的行并恢复 UPDATE 前的值。Normal Undo 删除、GlobalFinished 围栏写入和恢复终态使用同一个 COMMIT。独立查询确认原业务行恢复、相邻行不变，TC 全局会话、分支和锁清零。离线回放验证器拒绝缺少原复合主键锁定读取的报文。

| Rust | Commit | Rollback |
|---|---|---|
| 1.99 | [结果](evidence/java-tc-changed-commit-report/run-commit_mixed-1.99-ddbe5a0b/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_mixed-1.99-fa2a6d1e/mixed-packet-validation.json) |
| 1.85 | [结果](evidence/java-tc-changed-commit-report/run-commit_mixed-1.85-819b8623/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_mixed-1.85-516fc38f/mixed-packet-validation.json) |

### 脏数据回滚的状态映射

固定 Java 在锁定镜像发现脏数据后返回 `PhaseTwoRollbackFailedUnretryable`（10），要求人工处理。Rust 的 MySQL 适配器对控制 COMMIT 前确认的 `UndoDirty` 返回 `BranchResolution::ManualInterventionRequired`。RM 使用相同的线缆状态，并把文件日志留在 Pending，供同一分支显式恢复。连接故障和控制提交确认丢失继续返回可重试未知。已有不可逆拒绝仍使用 `Rejected`。

修复前，固定 Java TC 收到状态 9，并返回 `RollbackRetrying`。修复后，Rust 1.99 和 1.85 各以正式实验性一阶段接口建立真实 AT 分支。测试夹具改动业务现值后，原 TC 收到一次状态 10，TM 观测到 `RollbackFailed`（12），随后两个调度周期内无自动重试。独立 SQL 确认业务现值、原 Undo 七列、原回滚方向和历史一阶段提交见证均保留，文件仅有一条 Pending。受控真实 MySQL 用例进一步确认，校准业务值后显式恢复同一分支可完成原补偿并写入 Completed。两工具链的真实 MySQL COMMIT OK 丢失回归均仍返回状态 9 后由 TC 重试成功。

| 证据 | Rust 1.99 | Rust 1.85 |
|---|---|---|
| 固定 Java TC 脏数据回滚 | [结果](evidence/java-tc-changed-commit-report/run-rollback_dirty-1.99-c7cb6d02/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_dirty-1.85-72f37d99/verified.json) |
| 校准后显式恢复 | [结果](evidence/mysql-undo-rollback/run-dirty-recovery-5540657785c5/1.99.0/managed.log) | [结果](evidence/mysql-undo-rollback/run-dirty-recovery-5540657785c5/1.85.0/managed.log) |
| 控制 COMMIT OK 丢失 | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_drop_ok-1.99-ff15c7af/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_drop_ok-1.85-b320d523/verified.json) |

[修复前实际失败](evidence/java-tc-changed-commit-report/run-rollback_dirty-1.99-6923648e/client-stdout.log)。

### MySQL 控制提交断连

固定原 Java TC 通过直连 MySQL 8.4.13 存储全局事务和锁。正式 Rust RM 通过独立的回环代理连接同一自有数据库，使用实验性接口执行原一阶段 AT 提交。代理针对二阶段恢复表的终态语句定位同一物理控制事务，在四个时点分别断开连接。时点为终态写入尚未转发、COMMIT 尚未转发、服务端正在执行 COMMIT，以及收到真实 MySQL OK 之后。代理在关闭目标连接前通过另一条独立连接读取业务、Undo、持久方向、终态与一阶段见证。

当终态写入或 COMMIT 未被转发时，第一次提交效果尚不可见，Java TC 后续重试才完成原清理或补偿。丢弃已收到的 MySQL OK 时，首次提交效果已可见，后续回调确认原持久终态。验证器将真实预编译参数绑定到原 XID 和 BranchId，核对同一物理事务完成清理或补偿并写入终态，所有场景均只有一次已确认的 MySQL 二阶段效果提交。原始 Seata 帧与 RM 文件校验还核对一次全局结束请求，以及原分支仅有一次 Pending 和一次 Completed。

在途 COMMIT 用例只对新建的测试容器设置一秒 binlog 组提交延迟。独立查询在同一条 SQL 中核对原物理连接正执行 `COMMIT`，状态为 `waiting for handler commit`，并确认写入尚不可见。代理在组提交延迟内关闭两端连接。TC 重新发出原回调后，RM 按原分支的持久状态确认结果。

| Rust 与原指令 | 终态写入前 | COMMIT 未转发 | COMMIT 在途 | MySQL OK 已收到 |
|---|---|---|---|---|
| 1.99 Commit | [结果](evidence/java-tc-changed-commit-report/run-commit_db_block_terminal-1.99-2ea5d0cd/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-commit_db_block_commit-1.99-8ee06103/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-commit_db_in_flight-1.99-9ce22854/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-commit_db_drop_ok-1.99-f436cd60/verified.json) |
| 1.99 Rollback | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_block_terminal-1.99-9e476803/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_block_commit-1.99-239e44f0/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_in_flight-1.99-57a1baf1/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_drop_ok-1.99-c627b23e/verified.json) |
| 1.85 Commit | [结果](evidence/java-tc-changed-commit-report/run-commit_db_block_terminal-1.85-ed8565e4/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-commit_db_block_commit-1.85-6417899c/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-commit_db_in_flight-1.85-33f17acf/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-commit_db_drop_ok-1.85-2c84ff1d/verified.json) |
| 1.85 Rollback | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_block_terminal-1.85-348b060b/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_block_commit-1.85-6f28c160/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_in_flight-1.85-a2ae6922/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_db_drop_ok-1.85-1fc326f5/verified.json) |

### 二阶段成功回包丢失及迟到的相反指令

固定原 Java TC 使用 MySQL DB 存储。已登记的正式 Rust RM 通过实验性接口完成业务、Normal Undo 和一次 `PhaseOneDone`。联试观察器扣留首个 Commit 或 Rollback 二阶段成功回包并关闭原 RM 连接。RM 在新连接上成功登记资源后，原 TC 重试同一 XID 和 BranchId，并收到与首个回包相同的成功状态。TM 只发送一次原全局结束请求。若该请求缺少确认，TM 保留未知，并通过查询原 XID 观察 TC 终态。

原始帧、独立 MySQL 查询与文件日志核对分支身份、两次回包的实际送达、以及一次 Pending 和一次 Completed。提交场景保留业务行并清除 Undo，回滚场景清除业务行和 Normal Undo，并保留原防悬挂围栏。两种场景均清除 TC 全局会话、分支与锁记录。

| Rust | Commit 回包丢失 | Rollback 回包丢失 |
|---|---|---|
| 1.99 | [结果](evidence/java-tc-changed-commit-report/run-commit_callback_reply_loss-1.99-235aafe6/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_callback_reply_loss-1.99-bd6bf240/verified.json) |
| 1.85 | [结果](evidence/java-tc-changed-commit-report/run-commit_callback_reply_loss-1.85-f71d46b1/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_callback_reply_loss-1.85-175f2e4e/verified.json) |

另用受控 TCP 对端和真实 MySQL 发送重复指令及迟到的相反指令。提交已完成时迟到 Rollback 返回 `PhaseTwoRollbackFailedUnretryable`，回滚已完成时迟到 Commit 返回 `PhaseTwoCommitFailedUnretryable`。独立 SQL 确认方向、原终态和业务结果保持一致，回滚围栏的七列内容保持原样。[Rust 1.99](evidence/mysql-undo-rollback/run-phase2-direction-0b957f1e8f97/1.99.0/) · [Rust 1.85](evidence/mysql-undo-rollback/run-phase2-direction-0b957f1e8f97/1.85.0/)。

### 一阶段回滚后缺失 Normal Undo

在固定原 Java TC 的 MySQL DB 存储配置下，Rust RM 完成原分支登记，实际写入业务行和未提交的 Normal Undo，再显式回滚本地事务。独立连接先以 READ UNCOMMITTED 读取原 Normal Undo 的正文和上下文，随后确认本地回滚后业务和 Undo 均为零。原始帧显示 Rust TM 只发送一次 GlobalRollback，RM 发送零次 `PhaseOneDone`。迁移 004 的本地提交见证始终为零。

MySQL 原始语句和绑定参数显示，RM 锁定读取原 XID/BranchId，插入一次 `GlobalFinished`，检查孤儿片并在同一物理事务更新回滚终态。该事务执行零条业务补偿。首个状态 8 正常送达时，原 TC 完成一次回调。首个状态 8 被扣留并断开连接时，原 TC 等待 RM 在新连接上完成资源登记，再对同一 XID 和 BranchId 发出回调，收到第二个状态 8。TM 对未收到响应的原结束请求保留未知结果，随后只查询原 XID。两条路径都只写入一次 Pending 和一次 Completed，最终清空 TC 全局会话、分支和锁记录。

夹具向同一身份重放此前观察到的原 Normal Undo 字节，MySQL 返回 1062。独立查询确认围栏的七列内容与拒绝前完全一致。孤儿子片与已提交 Normal Undo 的恢复分别见相邻场景。

| Rust | 回包正常送达 | 首个成功回包丢失 |
|---|---|---|
| 1.99 | [结果](evidence/java-tc-changed-commit-report/run-rollback_missing_undo-1.99-4ce2d539/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_missing_undo_reply_loss-1.99-0cc29cc8/verified.json) |
| 1.85 | [结果](evidence/java-tc-changed-commit-report/run-rollback_missing_undo-1.85-32c5e039/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_missing_undo_reply_loss-1.85-2aaab5fb/verified.json) |

### 孤儿 Undo 子片的原分支重试

固定原 Java TC 使用 MySQL DB 存储。正式 Rust RM 通过实验性一阶段提交接口提交业务和 Normal Undo，确认一次 `PhaseOneDone`。自有测试库随后把原根行移为带原父号的孤儿片，分别测试根缺失和原根位置已有 `GlobalFinished` 围栏。首次回调返回可重试状态 9，TC 返回 `RollbackRetrying`，恢复行保持非终态。独立 SQL 读取的 Undo 七列与故障前一致，业务行仍在。夹具在单个管理事务中恢复原记录后，TC 对同一 XID 和 BranchId 重试，收到状态 8 并返回 `Finished`。

原始帧校验只发生一次全局回滚和一次分支登记。文件日志按原回调载荷核验 CRC，只包含一次 Pending 和一次 Completed。独立 SQL 确认业务行、Normal Undo、TC 全局会话、分支和锁均清零，仅保留原分支的防悬挂围栏。此场景使用自有夹具修复数据和测试认证配置。

| Rust | 根缺失 | 根位置已有围栏 |
|---|---|---|
| 1.99 | [结果](evidence/java-tc-changed-commit-report/run-rollback_orphan_missing_root-1.99-6b2355ba/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_orphan_guarded_root-1.99-3911e4ee/verified.json) |
| 1.85 | [结果](evidence/java-tc-changed-commit-report/run-rollback_orphan_missing_root-1.85-b4c58126/verified.json) | [结果](evidence/java-tc-changed-commit-report/run-rollback_orphan_guarded_root-1.85-4c19cbdb/verified.json) |

### 补偿连接获取失败

固定 Java TC 使用 FILE 存储，正式 Rust TM 和 RM 连接回环测试实例。TM 仅发送一次全局回滚。MySQL 在来源确认及方向绑定之后拒绝第一次补偿连接，原 TC 收到可重试状态 9，并对同一 XID、分支号及 RM 连接发出第二次回调。RM 返回状态 8 后，原 XID 的状态查询返回 Finished。首次故障前后的独立 MySQL 查询得到相同的 Undo 七列，业务行保持一条。最终业务行和 Undo 清零，恢复行进入终态。独立文件解析验证了原分支仅写入一次 Pending 和一次 Completed。

Rust 1.99 与 1.85 均实测通过，五组负控分别核对回调重试、身份、最终查询关联及日志校验。已提交初态由自有管理事务重建，Java TC 使用测试认证 SPI。公开一阶段提交、生产认证及其他 TC 存储模式仍待验收。 [固定 Java TC 重试验证](wp05-real-java-tc-compensation-retry.json)
