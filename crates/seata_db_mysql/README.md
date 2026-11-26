# seata-db-mysql

WP05 真实 SQLx/MySQL 基础与受限本地 DML：有界事务、MDL/完整元数据、物理域、前后镜像、Undo 准备和持久化。无变化且无 TC 分支的 `commit_no_change` 已有受限入口；有变化业务的 `commit_changed` 仍在默认关闭的实验特性下，不是生产 COMMIT 能力。

[最新执行证据](../../docs/phase-2/evidence/mysql-execution-gate/README.md)与[契约](../../docs/phase-2/mysql-execution-images-contract.md)列明显式PK INSERT、UPDATE/DELETE、预算、取消/超时和失败回滚边界。INSERT内部临时键赋值需要CREATE TEMPORARY TABLES；不自动授权，不猜自增键。原[前镜像](../../docs/phase-2/evidence/mysql-row/README.md)及[查询间竞争](../../docs/phase-2/evidence/mysql-image-race-gate/README.md)保留历史执行范围。

固定 MySQL 8.4.13、`lower_case_table_names=2` 的选定现场用例覆盖 Java Undo 格式、同事务落库、原 TC 分支登记、提交尝试和本地结果恢复；[一阶段提交契约](../../docs/phase-2/mysql-first-phase-commit-contract.md)列出原回执与 004/006/007 持久边界。外部写入器锁覆盖、完整 TM 重建、全部 SQL/Undo 源案例、生产 TLS/最小权限及 WP05/06 完整门禁仍未完成。


[原生类型身份](../../docs/phase-2/evidence/mysql-type-gate/README.md)保留 18 种 `DATA_TYPE` 及 tiny 显示宽度，不转换原 `FieldValue`；该历史检查点的后续 codec/落库工作已在上述选定场景推进，不能据此声称完整 Java 源案例覆盖。

WP07 MySQL 连接配置增量：`MySqlOptions::new` 在连库前拒绝 `0.0.0.0`、`::` 等未指定目的 IP 和全空白用户名，保留合法主机名及字面 loopback 测试 TLS 选择。Rust 1.99/1.85 的 5 项配置契约测试、45 项库测试与全目标严格 Clippy 均通过。此校验不代替真实 TLS/认证握手和生产部署发现；有变化提交仍由默认关闭的 `experimental_changed_commit` 提供，WP05/06/07 Gate 未闭合。


## 恢复控制事务只读指标

MySqlDataSource::recovery_metrics_sampler 与 MySqlDataSourceManager::recovery_metrics_sampler 签发 MySqlRecoveryMetricsSampler。同一数据源及其全部克隆共享六种 MySqlRecoveryOperation 的固定计数；重建管理器不清零，新建数据源归零。采样器不持有连接池、资源或恢复权限，数据源关闭后仍可读取。

MySqlControlMetrics 的 active 是在途调用；completed、unknown_commit、before_commit_error、cancelled 互斥结算一次内部 control 调用。completed 不证明业务提交，可能只是持久登记、方向绑定或只读事务。成功结果的 applied/unresolved 单独计数，Applied 包含读取已有终态，不能作为效果执行次数。取消不推断提交边界，提交前错误不证明旧业务没有效果。快照逐字段读取，累计饱和不回绕。

seata-observability 显式启用 mysql 特性后，with_mysql_recovery 可独立导出固定 42 条序列。默认导出器不引入 MySQL/SQLx 运行依赖。此增量覆盖恢复管理器控制事务；一阶段业务提交、独立 Undo API 和完整生产观测门禁仍需分别验收。见 [真实数据库与原 TC 证据](../../docs/phase-2/evidence/mysql-control-metrics/README.md)。
