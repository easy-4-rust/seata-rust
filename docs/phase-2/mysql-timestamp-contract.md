# WP05：原生 TIMESTAMP 与显式 UTC

本变更落实原架构 9.3 和[时间类型兼容契约](mysql-temporal-compatibility-contract.md)中的 TIMESTAMP 要求。DATE 的历法缺口、TIME 的时长/精度差异及完整 WP05 仍须完成，不以 TIMESTAMP 代替完整时间类型。

## 实施契约

TIMESTAMP 使用独立的 `Timestamp`、`ColumnType`、`ColumnDomain` 和 `MySqlType`。值是 UTC 时间点而非 DATETIME 的本地日历；允许精确到微秒，物理小数秒精度 0–6 按真实元数据校验。目标 MySQL 的已认证非零范围为 UTC 纪元秒 1–2147483647 及合法秒内微秒。拒绝零日期、零秒、负数、超界、非法日历及亚微秒输入；SQL NULL 独立保留。不得把不可表达的原值归一化成 NULL。

MySQL 手册的小数秒范围曾存在[官方记录的描述不一致](https://bugs.mysql.com/bug.php?id=112190)。这里由实际 MySQL 8.4.13 验证 `.999999` 上界及两侧非法值，不按文档中的 `.499999` 字样缩小值域；时间点与会话转换语义见[官方类型说明](https://dev.mysql.com/doc/refman/8.0/en/datetime.html)。

采用已存在的显式 UTC 配置 3/4，保留旧编号 1/2 及旧记录行为。原 Java 同资源的 TIMESTAMP 锁键要求 JVM UTC；Jackson 的 LocalDateTime UTC 设置保持原契约。配置条件应在部署时一致，不能只设置 Java parser 时区就推断 `Timestamp#toString` 也是 UTC。

所有正式 MySQL 连接配置明确选择 `+00:00`，包括从连接池配置创建的独立观察连接。读取 TIMESTAMP 元数据时，在同一独占业务/回调/恢复事务上读取 `@@session.time_zone`，仅接受明确的 `+00:00`。偏移会话和 `SYSTEM` 均不能冒充已确认 UTC，即使服务器当前系统时区恰巧是 UTC。该检查发生在生产 INSERT、UPDATE、DELETE 的业务执行之前。

持 MDL 核对原物理类型后，使用有界 CHAR 投影读取完整 UTC 日历，再构造独立时间点。该路径保持 SQL NULL，避免驱动把零日期隐式转成 NULL。普通 TEXT/VARCHAR 不取得 TIMESTAMP 原生身份。参数绑定与补偿都使用完整 UTC 文本；主键暂存使用同精度的真实 TIMESTAMP 列，并在同一 UTC 事务中读取最终主键值。

原 Jackson 格式为 JDBC 93、`java.sql.Timestamp` 包装：整毫秒值为纪元毫秒整数，存在亚毫秒时为 `[millis,nanos]`。原 `Timestamp#setNanos` 覆盖整个秒内纳秒，不采用 DATETIME/LocalDateTime 的追加余数规则。恢复后必须按原类、原元数据及配置重新编码核对，拒绝伪造类名、不一致的毫秒/纳秒、精度和类型。Java 主键字符串保留空格、完整秒和小数点，零小数使用 `.0`，其余小数删除末尾零；不能使用 LocalDateTime 的 `T` 或省略秒规则。

Undo 格式、SHA-256 原事务持久化、分支/资源身份、恢复与回调授权、幂等和默认有变化 COMMIT 开关沿用现有契约，不新增数据库迁移或依赖。

## 验收

先确认旧元数据识别对 TIMESTAMP 的真实失败，再核对值域与精度、原类/原格式、混淆类与非法载荷拒绝及锁键。原 Java 18 个固定源码重新编译，使用 Connector/J 8.0.27/Jackson 2.18.3/JDK 17.0.12 对照真实 MySQL 8.4.13。

Rust 1.99/1.85 × 配置 3/4 × Commit/Rollback 的正式 TC 矩阵，包含 0–6 精度、SQL NULL、上下界、显式 TIMESTAMP 主键的 INSERT/UPDATE/DELETE；核对原格式、TC 原帧、原事务 Undo/SHA、错误旧配置拒绝、实际补偿和原生分支/日志终态。每个现场还实际验证 UTC、+08:00、SYSTEM 会话的元数据准入，并验证非法日期、上下界外及零值。通过旧 DATETIME、JSON、普通 AT 回归及双 Rust 工作区测试和严格 Clippy，独立检查原始证据后才可标记本能力完成。

该矩阵不替代进程故障重启、所有时区、所有 MySQL/Java 版本、Fastjson 时间类型、完整 SQL 或生产准入；相应要求继续在原 WP05/WP07 和全计划范围中。

当前选定 UTC 原生读写、主键与正式回调已取得[现场证据](evidence/mysql-timestamp/README.md)和 [ledger](wp05-mysql-timestamp.json)。专门的 TIMESTAMP 跨进程重启恢复现已取得[强杀原进程、新 RM 接管的补验证据](evidence/mysql-timestamp-restart/README.md)。其他配置/格式、完整时间类型和 WP05 仍未关闭。
