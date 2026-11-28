# WP05 本地业务执行与前后镜像

沿用原[WP05实施契约](wp05-mysql-at-plan.md)及[已验证读取链](mysql-row-read-contract.md)。本增量推进真实INSERT/UPDATE/DELETE与内存SQLUndoLog准备，保持原提交/Undo/TC门槛。没有完整Undo序列化、兼容性、持久化或提交成功声明。

## 执行规则

- SQL仍必须来自正式不可伪造识别结果；原SQL及全部原参数以原顺序交给SQLx，只执行一次，不自动重放。完整操作共用一个绝对期限。无DDL/CALL/触发器/FK/自增/生成列等路径，沿既有拒绝矩阵。
- INSERT识别额外保存每个VALUES单元的已验证标量SQL模板、原参数起点/数量；表达式不能引用表列、函数或变量。模板只能由工厂构造，Debug隐藏字面量。所有PK必须显式提供。通过同连接私有InnoDB临时键表，按真实目标类型进行列赋值，不能用CAST推测转换；原单元参数切片原生绑定，不猜连续自增编号。取得完整PK并DROP TEMPORARY后，原INSERT只执行一次，再按PK取真实after image；before为空，实际插入与after行数必须吻合。
- UPDATE先取得完整before image、原PK和行锁，拒绝PK更新。执行原SQL后用before中的PK集合读取after，不能再使用可能已被SET改变的原WHERE。DELETE执行后原PK集合应为空。原PK顺序、原生参数、完整元数据与预检预算继续适用。
- 原DML不能因镜像重读被重复执行。任何执行、后镜像、身份/行数、期限或准备日志预算失败都poison事务，要求整体rollback/drop；不得保留部分镜像当成功。取消Future仍暂时拥有实际事务并丢弃连接。
- 返回SQLx原始affected_rows，不把它当成主键或镜像行数证明。UPDATE无变化、零匹配均须明确验证；before/after主键集合保持一致。

已核实物理字符集的文本字段保持 UTF-8 字节及原文本类型，MySQL 返回的二进制排序标记不改变字段的物理类型。真实 BINARY、VARBINARY 和 BLOB 保持二进制值。无效 UTF-8 仍须拒绝。真实 MySQL 8.4.13 验证覆盖 `utf8mb4_bin`、`utf8mb4_0900_bin`、`ascii_bin` 和 `utf8mb3_bin` 的 UPDATE、INSERT、原锁键及 Undo 回滚。 [验证记录](wp05-mysql-bin-text-images.json)

## 数据与预算

新增不可变SQLUndoLog保存SqlType及同一完整TableMeta下的before/after，table_name由元数据取得；INSERT前为空、DELETE后为空、UPDATE前后PK集合相同。它是原Java SQLUndoLog的部分实现，不是已经可由Java读取的日志格式。

每镜像仍最多1024行/8MiB payload、单字段1MiB；内部生成SQL最多1MiB，主键参数单元最多8192，全部预算同时生效。单事务最多准备128条日志、累计before+after payload16MiB；不将payload预算当成整个进程堆内存。超过预算不得提交，原本地事务要回滚。参数仍单字段1MiB/总8MiB；错误与Debug不输出原SQL/参数值。

`execute`只准备事务内效果及内存日志并返回原更新计数。当前公开事务仍只有读取/执行/回滚，未提供COMMIT；不能绕过Undo永久提交。后续必须完成原BranchRegister、同连接Undo唯一行及提交未知围栏后，才接入完整提交API。外部TC风险未接受、共享锁暂缓；原WP05十四项与WP06均不因此关闭。

内部临时键表只有受控ordinal列及原PK的对应类型列，不复制非PK默认值/触发器/外键，也不在临时键值上模拟业务唯一约束。仅使用CREATE/DROP TEMPORARY，不使用会隐式提交的ALTER/普通DDL；需要CREATE TEMPORARY TABLES权限。临时对象本身不随ROLLBACK撤销，成功路径显式删除，错误/取消使事务poison并依赖已有close_on_drop物理会话退出清理，不复用该连接。必须以先有本地业务效果再暂存/失败的现场验证无隐式提交；不因采用临时表就声称原子性已完成。依据MySQL8.4[临时表](https://dev.mysql.com/doc/refman/8.4/en/create-temporary-table.html)及[隐式提交](https://dev.mysql.com/doc/refman/8.4/en/implicit-commit.html)文档。

验证需要：显式复合PK多行INSERT及参数/常量混排、精确整数/DECIMAL/二进制/NULL；更新原WHERE后仍按PK捕获；DELETE缺失后镜像；零匹配/无变化；重复键、后镜像超限与真实驱动失败的整体回滚；独立连接证明无隐式提交、原业务执行恰一次；取消/超时、双Rust与实际削弱断言。Java案例/序列化差分、Undo原子提交与故障恢复继续原验收，不以rollback-only组件替代。


## 原生 JSON 列增量（2026-11-28）

非主键 JSON 已接入正式元数据、镜像、Java Undo 和实际补偿。保留服务器文本，区分 SQL NULL 与 JSON null；原生值类型与 1 MiB 预算分别验证。未扩大 SQL 语法。见 [契约](mysql-json-contract.md)、[双 Rust 原 Java / 原 TC 证据](evidence/mysql-json/README.md)。


## DATETIME 精确日历与主键（2026-11-28）

已实现 1000–9999、0–6 位精度、SQL NULL/零日期分离、同精度主键暂存及 Java 锁键。只有明确 UTC 配置可生成相应原 Java Undo；真实提交/补偿和旧 JSON/普通 AT 回归通过。[契约](mysql-datetime-contract.md)、[证据](evidence/mysql-datetime/README.md)。
