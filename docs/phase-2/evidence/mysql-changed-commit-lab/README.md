# WP05 有变化事务本地 COMMIT 实验

沿用 [一阶段提交契约](../../mysql-first-phase-commit-contract.md)。`seata-db-mysql` 的 `experimental_changed_commit` Cargo 特性默认关闭；开启后，只有消费原 `MySqlCommitCheckedTransaction` 才可调用 `commit_changed`。它要求原成功注册回执、非空 Undo、004 同事务候选见证及匹配的独立观察者。发送前不满足要求返回 `NotEligible` 并保留原回执；原 SQLx 事务在期限内发送一次 COMMIT，驱动失败或独立观察不确定都返回带回执的 `Unknown`。成功需要 COMMIT 确认与独立持久见证共同成立。该实验接口不向 TC 报告，不自动重放业务。

TDD 编译 RED 在 `changed-commit-red.log`：缺少 `commit_changed` 得到 E0599。自有 MySQL 8.4.13 上，正式准备路径先用受控原协议对端取得真实 BranchId=42，再执行原事务提交。成功用例核对业务、Normal Undo、004 见证均持久为 1、`mysql.general_log` 的原 COMMIT 增量恰好 1，并确认未向 TC 发报告。旧 003 表用例在发送前拒绝且业务与 Undo 回滚。断开原事务连接后返回未知并保留回执；另一个实验故障关闭匹配观察者的独立连接池，原业务事务实际持久提交后仍返回未知，健康观察者随后读到见证。完整 20 项仲裁现场回归在 Rust 1.99、1.85 各通过；最后新增的观察者故障用例在两版另行单测通过。各原始日志及默认/实验特性严格 Clippy 见本目录。第一次编译观察者故障用例时 D 盘构建缓存空间不足，测试未启动；使用 `cargo clean -p seata-db-mysql` 清理本项目可重建产物后重跑通过。`runtime.txt` 和 `cleanup.log` 留存自有容器与清理身份。

**这不是生产一阶段提交验收。** 测试注册端为受控原报文对端，未证明固定原 Java TC 对全部外部写入器的排他锁覆盖；实验接口未发送 `PhaseOne_Done`，未验证服务器 COMMIT 已生效但 OK 丢失、取消/进程退出、所有旧尝试围栏、业务行逐一重验或生产 TLS/多节点。默认构建仍不暴露有变化提交。原 WP05 十四项、WP06 和完整计划保持开放。
