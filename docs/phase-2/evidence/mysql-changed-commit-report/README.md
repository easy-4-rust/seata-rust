# WP05 实验有变化提交后一阶段报告

按[一阶段提交契约](../../mysql-first-phase-commit-contract.md)，默认关闭的 `experimental_changed_commit` 新增消费式 `commit_changed_and_report`。原本地 COMMIT 确认、独立持久见证和 Normal Undo 观察均成功后，才以原成功登记回执的 XID、BranchId、资源、应用数据和 AT 类型单次发送 `PhaseOne_Done`。报告失败保留原回执与 `RmCallError`，并单独标记本地提交已确认；本地提交未知不发成功报告。没有自动重复报告或业务 SQL。

受控原报文对端在收到报告的时刻另开 MySQL 连接，确认业务行、Normal Undo、004 见证均已持久化，然后分别返回成功响应或收到原请求后直接断连。断连路径返回报告结果未知，但本地提交事实仍确认。另在原业务连接被 `KILL CONNECTION` 后验证本地结果未知且对端没有收到报告。真实固定 Java TC 的报告状态、回调竞争和故障恢复仍未由这些受控对端案例证明。

| Rust | 报告成功 | 报告响应丢失 | 本地未知不报告 | 现场回归 | 严格 Clippy |
| --- | --- | --- | --- | --- | --- |
| 1.99 | `changed-report-1.99-success.log`：1/1 | `changed-report-1.99-loss.log`：1/1 | `changed-report-1.99-local-unknown.log`：1/1 | `changed-report-1.99-regression.log`：23/23 | 默认/实验均通过 |
| 1.85 | `changed-report-1.85-success.log`：1/1 | `changed-report-1.85-loss.log`：1/1 | `changed-report-1.85-local-unknown.log`：1/1 | `changed-report-1.85-regression.log`：23/23 | 默认/实验均通过 |

`changed-commit-report-red.log` 记录先写两个报告测试后，因方法不存在产生 E0599 的编译 RED；这不是报告时序的行为负控。初次现场运行缺少 `SEATA_UNDO_SCHEMA_PATH` 环境变量，测试在建表前失败；设置为 D 盘固定原 Undo DDL 后，双工具链用例及回归通过。`cargo fmt --all -- --check` 通过。`changed-report-cleanup.log` 记录在正确的 Ubuntu-20.04 WSL 中对精确自有容器和隔离引擎完成清理。上轮误查默认 Alpine 所写的运行时结论已在[原证据](../mysql-changed-commit-ok-loss/runtime.txt)更正。

默认配置的完整 `cargo test --workspace` 在两工具链各有 636 项通过、0 失败；原始输出为 `changed-report-1.99-workspace-test.log` 和 `changed-report-1.85-workspace-test.log`。这些默认测试不运行 `#[ignore]` 的现场用例，现场 23 项另见上表。

本功能仍为默认关闭的实验入口。尚缺原 Java TC 全部锁覆盖、旧尝试持久围栏、发送前明确失败报告、报告与回调竞争、取消/进程退出、生产认证及完整 WP05/WP06 Gate。不能把本轮通过解释为生产一阶段提交验收。
