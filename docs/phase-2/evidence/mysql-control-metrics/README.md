# MySQL 恢复控制事务指标验收（2026-11-27）

规格事实源是 [WP07 观测契约](../../wp07-observability-contract.md) 和 [V1 清单](../../../phase-0/v1-implementation-acceptance-checklist.md)。此增量不关闭 WP05–07 或整体计划。

同一数据源全部克隆共享 MySqlRecoveryMetricsSampler；管理器重建不清零，新建数据源归零。六种实际 control 动作的 active 与四种互斥结果分开，成功返回的 applied/unresolved 是附加观察计数。completed 不证明业务提交，Applied 包含读取已有终态，不能当成效果次数。取消不推断提交边界；采样器不持有池或恢复权限。显式 mysql 导出特性提供固定 42 条序列，默认运行依赖不引入 MySQL/SQLx。

## 目标行为和失败对照

- [基线代码 API 失败](baseline-api-red.log)：在保存当前文件后，临时以 HEAD 的三个原生产文件编译新用例，缺枚举和采样入口；finally 恢复原字节。这是对旧版本的明确回归验证。最初 [探针失败](red.log) 同时含测试字符串换行错误，不能单独作为纯行为 RED；已修正，保留原记录。
- 真实 MySQL 两用例分别核对：登记、方向绑定与补偿终态持久化；新管理器重复读取原终态只增加观察次数，实际行状态不变；InnoDB data_lock_waits 确认控制调用真正受阻后取消；池关闭后的新调用是 before_commit_error。双版本各 2/2：[1.99](1.99-live.log)、[1.85](1.85-live.log)。
- 真实数据源与 Bearer HTTP 每版本各 1/1：[1.99](1.99-http.log)、[1.85](1.85-http.log)。数据源关闭后控制失败只记 before_commit_error；采样器保留、恰好 42 条固定序列、错误令牌 401，以及身份/凭据哨兵不出现均有断言。
- 将 control 的结算改为直接丢弃守卫后，原真实 MySQL 用例在 completed/unresolved 为零处 [实际失败](settlement-mutation-red.log)；恢复原字节后 [同一用例通过](settlement-restored-green.log)。
- 并发饱和与未知分类两个单元用例已纳入双版本全仓回归；累计饱和不回绕，unknown_commit 不混入 completed 或 before_commit_error。
- 双版本全工作区全功能各 **707 passed / 0 failed / 317 ignored**：[1.99](1.99-workspace.log)、[1.85](1.85-workspace.log)。ignored 不算通过；上述数据库和下述 TC 现场另外显式执行。
- 严格全目标全功能 Clippy：[1.99](1.99-clippy.log)、[1.85](1.85-clippy.log)。默认功能严格 Clippy [通过](default-clippy.log)，默认运行依赖 [无 MySQL/SQLx](default-runtime-dependencies.log)。

自有 MySQL 8.4.13 使用固定镜像与原 Java schema SHA-256：[运行时](live-runtime.json)、[清理](live-cleanup.json)；源代码变异使用另一个 [运行时](mutation-runtime.json) 和 [清理](mutation-cleanup.json)。运行器仅操作自有随机命名容器、回环随机端口与独立数据库，失败也执行 finally 清理。

## 固定原 Java TC、COMMIT OK 丢失及原分支恢复

| Rust | 方向 | 完整现场 |
|---|---|---|
| 1.99 | Commit | [0f94e429](../java-tc-changed-commit-report/run-commit_db_drop_ok-1.99-0f94e429/verified.json) |
| 1.99 | Rollback | [ee753c23](../java-tc-changed-commit-report/run-rollback_db_drop_ok-1.99-ee753c23/verified.json) |
| 1.85 | Commit | [74f7ebe8](../java-tc-changed-commit-report/run-commit_db_drop_ok-1.85-74f7ebe8/verified.json) |
| 1.85 | Rollback | [bfc921c4](../java-tc-changed-commit-report/run-rollback_db_drop_ok-1.85-bfc921c4/verified.json) |

原 MySQL 已返回 COMMIT OK，但代理零投递，原控制调用 unknown_commit=1。TC 对相同分支重试；读取终态后 completed=1、applied=1，无第二次补偿或提交清理效果。一次 enroll、两次 observe、两次方向控制及两次终态控制按原始认证帧和固定字段逐项核对，原数据库、journal、终态与唯一效果检查继续执行；四组均有 stopped=true。

最终 [独立重放记录](control-replay.json) 绑定原帧与原指标 SHA-256，四组各拒绝八种削弱：未知伪成功、额外提交前错误、重复 Applied、身份标签、布尔伪整数、缺回调、响应早于请求、重试顺序倒置。最初三组完整现场各执行六个控制，重放补上两项顺序控制；最后一组完整现场直接执行八项。不覆盖原始记录。

## 诊断和自然重叠

- 首轮原 TC Commit（889d0c7b）的客户端指标正确，验收器漏算准备阶段真实调用的一次 enroll；源码 MySqlTransactionPreparer 和现场均证实它存在，修正期望后完整重跑。此轮未完成外部最终 SQL 检查，不计通过。
- 首轮 1.85 Rollback（c23d4d82）记录第二回调 index=8 在第一回包 index=9 之前到达；两原请求编号各有正确响应，计数与原帧一致。旧验收器强加严格串行假设而失败。db_fault 路径改为保留回调顺序和每个匹配响应晚于其请求的约束，完整身份、重试/完成状态和数据库唯一效果约束保持。该原始自然重叠记录通过最终八项指标反例重放，仍因初轮未执行外部最终 SQL 而不冒充第五次完整现场。
- fixture-name/schema/output-diagnostic.log 是运行器初始化约束诊断；fixture-field-clippy-diagnostic.log 是内部测试数据源新增字段漏接。均已修复，不计实际行为通过。

一阶段业务 COMMIT、独立 Undo API 的完整指标、生产采集/告警、全链路日志和整体 WP05–07 门禁仍开放。
