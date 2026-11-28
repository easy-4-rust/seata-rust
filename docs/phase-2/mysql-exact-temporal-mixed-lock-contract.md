# WP05：Java/Rust 精确 DATE/TIME 双向竞争

沿用完整 JDBC 配置准入及原 TC 锁域契约。选定配置 5/6 的实际 Java 写入器通过四参数守卫，在原 TC 登记并完成本地阶段一；原 Java RM 保持在线。两个正式 Rust 客户端使用同一物理 schema、原资源 ID、完整配置绑定和原 TC，分别以 DATE `1582-10-05`、TIME(6) `-25:01:02.123456` 主键执行有变化 UPDATE 并尝试登记。

验收要求：

- Java 锁未释放时，Rust 请求实际到达原 TC，并收到明确的锁冲突拒绝。不是本地配置检查拒绝。Rust 原事务回滚，Java 业务、原 Undo、分支与所有锁保持不变。
- Java 提交或回滚完成后，Rust 对相同键的正式准备成功；原 BranchRegister、branch_table、lock_table、正式登记收据中的资源与精确主键逐字一致。准备中的 Rust 变化对独立连接不可见。
- Rust 撤销尚未提交的准备事务并请求原全局回滚；回调按原 XID/branch ID 完成，global/branch/lock 表清空，正常 Undo 不遗留。空 Undo 回滚产生的原 `GlobalFinished` 围栏按唯一身份保留，不把正常围栏当作业务 Undo 泄漏。
- Rust 1.99/1.85、配置 5/6、Java commit/rollback 和 DATE/TIME 均实际运行，保存输入、原始帧、独立数据库观察、实际二进制及来源绑定。

已取得[Java 持锁方向的完整矩阵证据](evidence/java-writer-jdbc-profile-tc/README.md#java-持锁双-rust-竞争)：12 笔原 Java 全局事务和 32 笔 Rust 全局事务；60 次 Java 配置拒绝，16 次原 TC 对 Rust 的锁冲突拒绝，Java 释放后 16 次 Rust 正式登记。独立核验通过。

## Rust 已提交持锁、原 Java 竞争

反向要求 Rust 正式执行两个有变化 UPDATE，经完整配置准入、原登记、提交前门闩、原物理阶段一提交和成功报告后，保持原 TC 的 DATE/TIME 锁。独立连接必须同时看见业务、原 Normal Undo、正确配置码、阶段一见证以及与实际原 Undo 字节匹配的持久 SHA-256。

Java 使用真实 DataSourceProxy、精确数据源和四参数入口。持锁时，两个键的登记必须实际到达原 TC 并被锁冲突拒绝，Java 本地变化回滚，Rust 原分支、业务、Undo 与 checksum 不变。Rust 提交或补偿后，Java 对同键成功登记，写入原 Jackson Undo，并由原 Java RM 实际补偿恢复到 Rust 的最终结果。

阶段一提交是历史事实，补偿后见证仍为 1。原 TC 的重试可能产生重复补偿；所有原回调须逐个按物理连接/请求 ID 和 XID/branch ID 配对，正常 Undo 为零，合法 GlobalFinished 围栏按已完成分支身份核对。下一组自有测试的隔离清理不能当作系统自动清空 Undo 的证明。

[反向完整证据](evidence/exact-temporal-rust-owner-java/README.md)已覆盖双 Rust、配置 5/6、Rust commit/rollback，共八组、40 笔全局事务，Java 16 次冲突拒绝、16 次成功登记和实际补偿。由此补齐上述选定键的双向链路。生产认证、账号权限、所有写入器强制接入、全部 SQL 与 WP05 总门禁保持开放。
