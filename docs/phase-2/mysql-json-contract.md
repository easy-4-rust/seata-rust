# WP05：MySQL 原生 JSON 列

本变更落实原架构计划 9.3 的 JSON 列要求，沿用 WP05、既有镜像预算及 Java Undo 格式。增加非主键 JSON 列的正式读写、Undo 与补偿，不改变 SQL 语法白名单、主键、触发器、生成列、索引、事务及提交准入规则。

MySQL 将 JSON 以自身语义存储和规范化，文本转换使用 utf8mb4 / utf8mb4_bin；见[官方 JSON 说明](https://dev.mysql.com/doc/refman/8.4/en/json.html)。Rust 保存服务器返回的完整 UTF-8 文本，不把它解析成浮点数或重新序列化 JSON。保证的是原服务器镜像，而非输入文本的空格、键顺序或服务器自身不支持的数值精度。

物理元数据保留 MySQL JSON 身份。不得伪装为 LONGTEXT，也不得凭“都是字符串”猜测 JDBC 类型码。类型码、Java 值类与原 Jackson Undo 形状以固定原 Connector/J、TableRecords、JacksonUndoLogParser 的现场结果为准。SQL NULL 与文本 `null`（JSON null）必须区分；对象、数组、字符串、布尔、整数及 UTF-8 补充字符均保持原服务器字节。

字段仍受 1 MiB、镜像仍受既有行数/总字节预算。读取先核对真实行的 JSON 类型，再取原文本；不能通过放开所有 SQLx 类型检查来兼容。JSON 不授予主键能力。非法 JSON 由原服务器拒绝，原事务整体关闭，不能产生可提交的半成品。

验收：先由原实现运行 JSON 元数据/实际执行测试得到目标失败；固定原 Java 源码重新编译，记录原 JDBC 类型与值类、原 Undo 及实际类来源；Rust 1.99/1.85 通过真实镜像、Java Undo 对照、正式原 TC 提交/回滚、脏镜像拒绝、SQL NULL/JSON null 和预算拒绝。恢复、校验和及无关类型回归继续通过。不得因本类型实现关闭 WP05、生产准入或完整 V1 门禁。


选定配置已实现并验证，见 [原 Java / 原 TC / 双 Rust 现场](evidence/mysql-json/README.md) 与 [ledger](wp05-mysql-json.json)。完整 WP05 门禁保持开放。
