# XA COMMIT 响应丢失后的同事务见证恢复

事实源为 [XA 契约的见证增量](../../../phase-2/mysql-xa-contract.md)。最终目录、源码摘要及结果见 [machine ledger](../../mysql-xa-commit-witness.json)。第一阶段证据保留为历史记录，不覆盖原 Pending 结果。

015 显式创建独立 InnoDB 见证表。受管理 Prepare 在原 ACTIVE 连接上唯一写入原登记身份，再 END/PREPARE；见证与业务同属一个 XA，不另行提交。控制行锁保护写见证期间的原方向，元数据锁和独立约束检查保护表结构；原业务 DML 接口继续拒绝所有 seata_rs_ 内部表。

原生 Commit OK 被截留后，首次回调仍保留原 Pending。后续 Commit 恢复先核对原登记及持久方向；在原 Prepared 不可见时，只有精确匹配原 XID、BranchId、资源、可选应用数据、实例、版本和提交标记的已提交见证，才允许持久确认原终态。该路径不再发送 COMMIT，不重放业务。见证缺失继续 Unknown，Rollback 响应丢失的否定证明仍未实现。旧 014 分支不被补造见证；已有明确原生成功响应的结束路径继续有效。

最终现场：双 Rust 各八项受管理 MySQL 用例；每版截留真实原生 COMMIT OK，先观察 Pending，再在六类见证篡改、额外索引和见证移除下拒绝确认，恢复原字段后由新数据源及新管理器确认原 Commit，原 RM 下一次回调完成。独立 MySQL general_log 记录原生 COMMIT 恰好一次，实际业务、控制行、见证及 XA RECOVER 一致。Prepare 后见证不可见，正常 Rollback 不留下见证。十二项证据削弱检查被拒绝。

固定原 Java TC、原认证入口和真实 MySQL 下，双 Rust 各执行全局 Commit/Rollback；独立解析原始 V1 帧核对 XA 原登记、原 long BranchId、可选字段、回调状态及全局终态。工作区每版各 785 passed / 366 ignored，严格 XA/MySQL Clippy 通过。1.99 工作区回归之后只调整了现场夹具的等待/连接预算和换行保留；最终双版本现场重新编译、运行并封存全部当前源码。没有把 ignored 本身计为已通过的现场用例。

红测试 mysql-red_20261127_211726_c4dfa0fc 在未实现见证时精确复现第二回调仍为状态 6；实现后状态为 5。中间的 mysql_212651 因通用 EOF 短等待失败，mysql_212826 因故障代理累计 32 连接上限失败；两者保留为诊断。旧 Java 中间记录因现场夹具/换行改动触发源字节检查而未签发通过目录。1.85 回归中间记录 212856 遇到 Windows TCP 10053，最终完整回归 213103 通过。最终证据仅取 machine ledger 指定目录。

完整计划继续开放：持久生产者/结束意图、旧活动会话围栏、Rollback OK 丢失恢复、真实进程崩溃/并发/完整原 TC 故障门禁，以及 AT 生产验收、TCC、Phase 5–7。015 的部署、写入权限、保留策略和运行手册仍须纳入生产验收；运行时不建表、不修改 Java Undo 或旧 Undo 记录。
