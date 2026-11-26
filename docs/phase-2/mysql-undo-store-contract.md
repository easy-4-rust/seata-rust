# WP05 同连接Undo持久化

细化既有[WP05](wp05-mysql-at-plan.md)、[原格式](mysql-undo-format-contract.md)与[编解码](undo-codec-document-contract.md)，不改变已批准核心验收。当前公开事务仍不提供COMMIT；本步是完整AT提交链所需的存储部分，不能代替后续登记、持久围栏与提交未知恢复。

1. `MySqlLocalTransaction::flush_undo_logs(xid, branch_id, profile)`接收调用者原身份，在现有独占SQLx事务内写入固定原`undo_log`。当前方法不证明身份来自TC；正式提交入口必须由原BranchRegister结果绑定，不能直接接受应用自造身份。空准备日志不写表。成功后封存此作用域，禁止继续执行/重刷；失败或取消poison，整体回滚/丢弃实际连接。
2. 写入前持有Undo表MDL并核对InnoDB、七列、原唯一身份索引、无触发器/外键/生成列。真实业务与Undo共享同一本地事务，绝不创建第二连接写Undo、自动提交、忽略/覆盖重复身份或重发业务SQL。已有Normal或GlobalFinished行均由原唯一约束仲裁，原防悬挂行保持不变。

事务引擎还必须与原事务持有 MDL 时的当前物理建表定义核对。业务表可能先建立旧字典快照，不能把 information_schema 中的旧 InnoDB 字段当作当前事务引擎证明。当前引擎不符时在第一条 Undo 写入前拒绝，原业务作用域保持污染并整体回滚。

Normal 写入、围栏准备及已授权清理在原事务既有校验通过后，必须使用同认证和 TLS 配置的新连接再次执行现有 Undo 表结构规范。新连接核对原实例 UUID 与 schema，并在独立只读用途事务中读取当前引擎、七列、唯一身份索引、触发器及入出向外键；原事务持续持有目标 MDL。辅助事务只读取元数据并回滚，不读写业务行或发布原分支结果。连接、查询、辅助回滚和关闭均处于调用方的原绝对期限内，任何失败继续保留原错误和污染/清理语义。每次活动校验临时增加一个池外连接，稳定单主路由要求继续适用。

固定 MySQL 8.4.13 已复现旧事务漏过后提交的 Undo 触发器，业务和 Undo 回滚后仍留下一个非事务副作用行。新校验的七组现场用例在写入前拒绝迟到约束及结构变化，原作用域污染并整体回滚，各表无副作用。Rust 1.99、1.85 各通过 12 项写入、16 项补偿、9 项围栏、1 项受控原包提交清理回归，以及 635 项工作区测试与严格 Clippy。等待中的 DDL 可能令辅助 MDL 校验超时，仍由原期限和回滚策略安全终止。恢复表快照及完整 WP05/WP06 门槛继续实施。 [当前 Undo 规范验证](wp05-undo-current-schema.json)

固定 MySQL 8.4.13 已复现旧校验接受后来改成 MyISAM 的 Undo 表，业务回滚后仍残留一条非事务 Undo。当前物理引擎核对已拒绝该路径，三组原始建表字节进一步核对带完整伪装引擎词元的列/表注释。拒绝原因精确匹配当前物理引擎，回滚后业务及 Undo 均为零；恢复 InnoDB 后同一数据源可重新写入并整体回滚。两套 Rust 工具链各通过 11 项写入、16 项补偿、1 项业务引擎现场回归、635 项工作区测试及严格 Clippy。列、索引、约束及恢复表的其他快照维度继续分别验收。 [当前 Undo 引擎验证](wp05-undo-engine-ddl.json)
3. 使用已验证Java Jackson对象图与显式JDBC配置，原字段`branch_id/xid/context/rollback_info/log_status/log_created/log_modified`、Normal=0、服务器`now(6)`。当前显式NONE压缩配置；ZIP等原配置继续原完整迁移，不能声称支持。context保留serializer、compressorType、实际max_allowed_packet与分片subId；不改原schema或静默另建恢复表。
4. 按原MySQL manager的80%实际packet边界切片，后续分片先写、首片与subId上下文最后写，所有片均在同一事务内。分片子身份由本机MySQL的`UUID_SHORT()`分配并验证正有符号long、与原BranchId不同且本次不重复；这是Rust适配层的身份分配，不声称原Java UUIDGenerator算法已迁移。唯一冲突或无法表达的context保持失败并回滚，不重试/覆盖。原128字符context、32MiB codec及操作总期限仍限制完整工作；UUID_SHORT相关条件和重启/多节点原ID策略继续后续身份模块验收。
5. 用真实MySQL独立READ COMMITTED/READ UNCOMMITTED观察者与performance_schema锁事务号，证明业务/Undo在同一本地事务且未隐式提交。覆盖回滚、重复Normal/GlobalFinished、不安全/缺失表、分片及重组、context预算、实际锁等待期间取消/超时。错误不包含原SQL/业务值/原服务器消息。两版Rust、目标断言负控、原Java manager写入/读取对照与只读审查分别留证据。

全局排他锁的已确认原TC覆盖问题继续开放；本步不转义/hash原锁键、不接受外部TC风险，不允许绕过门槛永久提交业务。全原 JUnit 具体案例、WP05 十四项、WP06 与后续全部 Phase 仍继续。文本列采用二进制排序规则时的镜像和 Undo 恢复已在真实 MySQL 8.4.13 验证。 [验证记录](wp05-mysql-bin-text-images.json)
