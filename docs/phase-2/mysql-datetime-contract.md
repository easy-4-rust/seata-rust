# WP05 时间字段：DATETIME 实施契约

本变更落实原架构 9.3 的时间字段要求，沿用现有 WP05、Undo 和恢复契约。先完成 DATETIME 的合法年 1000–9999、0–6 位小数秒、可空字段、主键及真实补偿；DATE、TIME、TIMESTAMP 仍在完整时间类型范围，不能由 DATETIME 结果推断完成。

固定 Connector/J 8.0.27 的 DATETIME 需由原驱动确认 JDBC 编号和值类。原 Jackson 对 LocalDateTime 使用可配置时区，不能根据 Rust 主机时区或旧配置猜测。增加明确的 `Connector8DefaultUtc` / `Connector8NumericTinyIntUtc` 配置，要求对端 Java Undo 同样使用 UTC。持久编号分别为 3/4，旧 1/2 保留；错误配置必须在原分支身份检查时拒绝，不能重解释已存 Undo。切换部署配置仍须按现有规则排空旧分支。

使用有界、无浮点的日历值，保留微秒和 Gregorian 日历。Java LocalDateTime 的锁键字符串遵循其秒/小数省略规则；主键暂存列必须使用相同 DATETIME 精度。原数据库日期通过明确的 CHAR 投影读取日历字段，绑定使用完整日历文本；转换、零日期及精度均需真实 MySQL 验证。投影不赋予任意字符串原生时间身份。

默认 Jackson 格式使用原 JDBC/Java 类和 UTC 毫秒及亚毫秒数组规则；负纪元必须按向下取整保留秒内纳秒。日期不经过 JSON 浮点、宿主本地时间或隐式时区转换。SQL NULL 与零日期分离；有界日历值拒绝零日期、非法日历及超出六位精度的输入，不将其转成 NULL。普通 SQL 赋值的数据库转换沿用 MySQL 语义，并由实际 after image 记录。

验收包括：原实现对真实 DATETIME 的目标运行失败；固定原 Java 源码重新编译与 UTC 格式对照；全范围/闰年/1582 年/纪元前后/微秒边界；0–6 精度及主键登记；错误旧配置拒绝；非法日期、零日期和 NULL；Rust 1.99/1.85 的原 TC Commit/Rollback、Undo/见证/SHA、日志及实际业务核对；旧 JSON 和普通 AT 回归。保留旧记录、旧配置和默认提交开关，不关闭完整 WP05 或生产门禁。


选定 DATETIME UTC 配置已实现并验证：[现场与配置](evidence/mysql-datetime/README.md)、[ledger](wp05-mysql-datetime.json)。其余时间类型与完整门禁保持开放。
