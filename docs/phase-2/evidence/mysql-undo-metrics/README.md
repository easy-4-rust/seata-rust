# 独立 MySQL Undo 调用指标

本轮落实 [WP07 既有可观测契约](../../wp07-observability-contract.md) 的独立 Undo API 计量。生产 API 为 MySqlDataSource::undo_metrics_sampler()，可选 mysql 导出通过 MetricsSources::with_mysql_undo(...) 接入。采样能力只读，不拥有池、连接、原事务身份、关闭或恢复权限。

## 语义与边界

- 固定 operation=rollback|fence，同数据源全部克隆共享，新数据源归零，关闭后仍可读取。Future 尚未轮询不计；已轮询取消或 panic 解栈计 cancelled，不推断提交边界。
- completed、unknown_commit、before_commit_error、cancelled 各自结算一次。非法身份属于本次 COMMIT 前错误，不能推断旧业务没有效果。未知结果不计成功，即使服务端实际已提交。
- consumed、fenced_inserted、fenced_existing、undo_present 只统计 Ok 返回的原观察。既有围栏不计新效果。observed_restored_rows 仅累计 Ok(Consumed) 返回的行数，不涵盖 COMMIT 未知但已产生的真实补偿，不能作为全部数据库效果总数。
- 累计饱和不回绕，active 是 API 在途 gauge；快照逐字段读取，不保证跨字段原子一致。进程退出不保留这些内存计数。
- 新增固定 20 条序列：两条 active、八条结果、八条观察、两条 observed rows。标签仅固定动作与结果/观察，不含 XID、BranchId、SQL、资源 URL、凭据。默认导出器不增加 MySQL/SQLx 运行依赖。既有六动作恢复管理器的 42 条控制序列另算，不相加推断业务效果次数。

## 实际验证

run_20261127_035149_a0250459 包含 Rust 1.99/1.85 各六组现场：

1. 原 Normal 存在、消费一行、创建/读取围栏、重复围栏观察、输入错误、克隆共享、新数据源归零、关闭后可读及未轮询 Future 不计。
2. 实际 InnoDB 原 Undo 行锁等待中的取消，独立 SQL 确认等待后才取消；解除锁后同身份显式恢复。
3. 同一原锁等待的真实期限错误，计提交前错误，显式恢复后只累计明确观察到的一行。
4. 补偿 COMMIT 的原 MySQL OK 被代理截留：业务与原 Undo 实际已变更，但 unknown_commit=1、completed/consumed/observed rows 均零。后续同身份观察产生围栏，原未知计数不被改写为成功。
5. 围栏 COMMIT 的原 OK 被截留：实际 guard 存在；后续同身份读取只计 fenced_existing。
6. 真实 Bearer HTTP：错误凭据拒绝，全部 20 条零值序列、成功/错误计数及关闭后的最终读数准确，敏感哨兵不出现。

上述四次 OK 丢失均保留原 COM_QUERY COMMIT、服务端 OK 原字节、摘要和单连接因果顺序。只读重放 validation_20261127_040004_05a754e1 各版本拒绝 12 种削弱，包括未知伪成功、误计恢复行、重复观察、泄漏身份、布尔伪计数、取消伪成功、虚构 OK 截留及错误包摘要/顺序。原报文观察器未修改，最终复核封存从 Git 9180cddf 继承的精确源字节。

删除正式 fence 结算的源代码变异，在真实 API 的 completed 计数断言处失败（0 对 3）；finally 恢复原字节后同一目标通过。实际业务/API 结果不被计数器修改。自有随机名 MySQL 容器按镜像与 owner 标签核对后清理；最终独立查询零残留物理会话，D 盘原数据留作诊断。

全工作区全功能两个版本各 726 passed / 320 ignored（含 rustdoc 汇总），MySQL 与 observability 全目标全功能严格 Clippy 通过。三个纯计数用例覆盖未知分类、饱和、克隆并发、取消/panic。默认导出器严格 Clippy 通过，正常依赖树没有 MySQL/SQLx。ignored 不计通过，上述现场另行显式运行。

## 证据与重跑

权威入口：[wp07-mysql-undo-metrics.json](../../wp07-mysql-undo-metrics.json)，每个目录保存原输出及 catalog 摘要。

    python -B tools/formal-source-junit/mysql_undo_metrics_live.py
    python -B tools/formal-source-junit/mysql_undo_metrics_regression.py --rust 1.99
    python -B tools/formal-source-junit/mysql_undo_metrics_regression.py --rust 1.85
    python -B tools/formal-source-junit/mysql_undo_metrics_default.py
    python -B tools/formal-source-junit/validate_mysql_undo_metrics.py --evidence docs/phase-2/evidence/mysql-undo-metrics/run_20261127_035149_a0250459 --catalog-sha256 df1d7c6232834997daf6a3c2711bf1173626aad938a3181da0fdbe59a16c7f9f

变异现场不与其他 Rust 构建并行运行；只有最后两个工具链的独立 target 回归可在没有变异时并行。运行器只使用既有锁定依赖、已安装固定 MySQL 镜像及已封存原 Java schema，开发写入限定 D/E。

## 未完成

独立人工/另一 Agent 审查、一阶段业务全指标、生产日志采集、远程认证/TLS 代理、告警及负载/安全门禁仍开放。本批不关闭 WP05–07、完整 V1、其他原案例迁移或后续 Phase。
