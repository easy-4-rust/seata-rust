# WP05：Java 持锁时双 Rust 的精确 DATE/TIME 竞争

沿用完整 JDBC 配置准入及原 TC 锁域契约。选定配置 5/6 的实际 Java 写入器通过四参数守卫，在原 TC 登记并完成本地阶段一；原 Java RM 保持在线。两个正式 Rust 客户端使用同一物理 schema、原资源 ID、完整配置绑定和原 TC，分别以 DATE `1582-10-05`、TIME(6) `-25:01:02.123456` 主键执行有变化 UPDATE 并尝试登记。

验收要求：

- Java 锁未释放时，Rust 请求实际到达原 TC，并收到明确的锁冲突拒绝。不是本地配置检查拒绝。Rust 原事务回滚，Java 业务、原 Undo、分支与所有锁保持不变。
- Java 提交或回滚完成后，Rust 对相同键的正式准备成功；原 BranchRegister、branch_table、lock_table、正式登记收据中的资源与精确主键逐字一致。准备中的 Rust 变化对独立连接不可见。
- Rust 撤销尚未提交的准备事务并请求原全局回滚；回调按原 XID/branch ID 完成，global/branch/lock 表清空，正常 Undo 不遗留。空 Undo 回滚产生的原 `GlobalFinished` 围栏按唯一身份保留，不把正常围栏当作业务 Undo 泄漏。
- Rust 1.99/1.85、配置 5/6、Java commit/rollback 和 DATE/TIME 均实际运行，保存输入、原始帧、独立数据库观察、实际二进制及来源绑定。

已取得[完整矩阵证据](evidence/java-writer-jdbc-profile-tc/README.md#java-持锁双-rust-竞争)：12 笔原 Java 全局事务和 32 笔 Rust 全局事务；60 次 Java 配置拒绝，16 次原 TC 对 Rust 的锁冲突拒绝，Java 释放后 16 次 Rust 正式登记。独立核验通过。当前方向为 Java 持锁、Rust 竞争；Rust 已提交持锁后 Java 竞争仍待单独核验。生产认证、账号权限、所有写入器强制接入、全部 SQL 与 WP05 总门禁保持开放。
