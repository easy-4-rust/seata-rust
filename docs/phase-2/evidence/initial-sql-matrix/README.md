# 首期 SQL 可执行矩阵与原 TC 验收

原 22 项历史权威包：[20261128_135612_0865d849](20261128_135612_0865d849/result.json)。catalog SHA-256：`e980032696b95661939b4104cdb5be6d467eb9bae84ca3daf771c19b09e1d437`。规格仍为[原 WP05 首期矩阵](../../wp05-mysql-at-plan.md#sql识别第一轮矩阵数据库执行尚未认证)，[可执行资产](../../../../crates/seata_db_mysql/tests/data/mysql_sql_matrix.json)列出原 SQL、参数及独立预期行。下文旧计数对应当时的封存源码；当前 28 项与别名新增验收见文末。

| 范围 | 实际用例 |
|---|---|
| INSERT | 单行、多行 VALUES、常量/原参数混排、显式列重排、NULL、文本中的问号 |
| UPDATE | 原 WHERE 值被改变、无 WHERE、加减乘除/取模、正负一元运算、括号、布尔常量、反引号列及转义单引号文本 |
| WHERE / DELETE | 六种比较、AND/OR/NOT、IN/NOT IN、BETWEEN/NOT BETWEEN、IS NULL/IS NOT NULL、参数、全表/范围/IN 删除 |
| 识别拒绝 | 22 个已声明不支持的语法输入，包含 DDL/事务控制/CALL/SELECT/REPLACE/UPSERT/IGNORE/INSERT SELECT/RETURNING/CTE/子查询/JOIN/别名/跨 schema/排序限量/函数变量/注释/反斜线/多句/VALUES ROW |
| 元数据拒绝 | 解析成功的主键 UPDATE，在实际表元数据核对时返回 `primary key update`，原本地事务回滚后行保持原值 |

Rust 1.99.0、1.85.0 各执行一遍提交和补偿流，每流 22 个不同全局事务，共 88 笔。每个用例独立建表/种子并实际执行原 DML；外部连接在提交前只能看见原种子，阶段一后可见独立预期值和对应原 Normal Undo。原 TC 的精确资源、XID、branch ID、锁行和一次成功报告均由原始字节及独立 SQL 核验。

阶段一同时持久化原 Undo、SHA-256、配置码和提交见证；结束后恢复记录的终态、摘要及历史见证保持正确。提交保留预期业务，补偿逐条恢复种子；TC global/branch/lock 和正常 Undo 为零。保留围栏必须属于已完成的原分支，单独记录，不把围栏当作正常 Undo 泄漏。

固定原 Java `JacksonUndoLogParser` 实际读取全部 88 份日志，并重编码核对全部业务字段、JDBC 类型及原身份。比较仅忽略 Java 集合实现标签和结构 `@class`；不忽略字段、SQL 类型、表名、主键、值、NULL 或行顺序。该现场验证原 parser 格式读取，业务补偿由正式 Rust RM 执行，不冒充 Java RM 对全部句式的执行。

```powershell
python -B tools/formal-source-junit/native_sql_matrix.py
python -B tools/formal-source-junit/validate_sql_alias_reference.py docs/phase-2/evidence/initial-sql-matrix/20261128_144825_98e36d0c
```

包内保存完整源码输入、双版本分类测试、构建/严格 Clippy 原始结果、二进制绑定、原 Java 编译类和实际来源、原协议帧、所有 Undo 与重编码、逐条独立业务/锁/摘要/终态查询。`135141` 是较早单版本回滚原型，未覆盖最后加入的执行拒绝及恢复记录核验，不替代完整包。

固定原 TC 使用自有 DB backend、1 秒清理阈值，认证关闭、数据库使用夹具 root。此证据不关闭全部 Java SQL、全部物理类型/外部写入器、生产权限/认证、默认业务 COMMIT、故障矩阵或 WP05 总门禁。

长于一分钟的旧现场记录了 TC 发出的原生单向 `UndoLogDeleteRequest`（111，AT、同资源、7 天）。当时 RM 只执行同步分支回调；后续[定期清理](../undo-retention/README.md)已独立实现并验收。旧现场本身不把已收到清理指令当作清理已完成，也不据此发布生产运维能力。

## 单表别名与完整原矩阵回归

当前权威包：[20261128_144825_98e36d0c](20261128_144825_98e36d0c/result.json)，最终 catalog SHA-256：`c11d01df588ee3566f6d1e67b3bb379d39032022c4dca73fcd227bdcb049972e`。按[别名契约](../../mysql-sql-alias-contract.md)增加六个真实用例：显式 AS UPDATE、隐式反引号别名、无 WHERE UPDATE、显式/隐式 DELETE、无别名真实表限定列。旧别名拒绝项替换为未知前缀拒绝，保留其余原用例。

双 Rust 1.99/1.85 × 提交/补偿共 112 笔实际全局事务；28 接受、22 语法拒绝、1 元数据拒绝。全部用例独立核验原 SQL 实际结果、精确锁键、一次阶段一报告、原子 Undo/checksum、提交历史、真实回调与最终业务。原 Java parser 重读全部 112 份日志。别名没有进入持久表名、锁键和 Undo 表身份。

补充运行原 Java Druid 识别器，六个新增输入的真实表、别名、SQL 类型与去掉合法前缀后的赋值列一致；保留原反引号和带前缀列的实际输出，不修改原实现。源码 pin、原 JAR、执行 class、实际类加载来源及结果由 `alias-reference/` 保存。每版另通过 128 个 AT 测试、12 个忽略，以及 AT/MySQL 的严格 all-target Clippy。忽略项未计通过。`validate_sql_alias_reference.py` 先完整核验 112 笔原生证据，再核对这组原 Java 识别器与组件结果。

完整 Java SQL（JOIN、子查询等）、全部写入器和生产准入仍未完成，不关闭 WP05/WP07 或默认有变化 COMMIT 门禁。
