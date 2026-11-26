# WP07 TM 调用结果指标

规格事实源：[WP07 可观测性契约](../../wp07-observability-contract.md)与[V1 验收项](../../../phase-0/v1-implementation-acceptance-checklist.md)。不改变原错误、恢复能力、TC 状态或业务重试语义。

## 实现与结果边界

每个 TransactionManager 持有固定四种操作的共享原子计数，通过 metrics_sampler 签发仅能读取的 TmMetricsSampler。TmCallGuard 在首次轮询进入 Begin 或 End/Status 路径时增加 active，返回或被取消时归还。正常结果分为 observed、tc_rejected、unknown、not_sent、local_rejected；未返回结果的退出只记 cancelled。协议错误另记 protocol_errors，写后协议错误仍同时属于 unknown，不伪装成权威成功。

observed 包含合法但尚未终结的状态，不能称为全局事务已最终完成。参与方不发送的结束、持久来源存储阶段和业务自身错误不作为原生调用结果统计。计数不含身份，不跨管理器或进程持久化；关闭管理器后只读采样器仍保留最终值。并发逐字段采样不是原子总表，累计值饱和不回绕。

MetricsSources::with_tm_calls 接入现有回环 Bearer 端点，兼容原连接采样构造器，也可单独导出调用计数。四种 operation、六种 outcome 加 active 与 protocol_errors 一共 32 条固定序列，不接受自定义标签。认证及请求预算保持原行为。

## TCP、HTTP 与边界测试

首次测试分别在缺 metrics_sampler 和 with_tm_calls 时实际编译失败：[TM RED](red.log)、[HTTP RED](export-red.log)。之后 5 项真实 TCP 测试验证有效响应、Failed、丢响应、原 Commit/Rollback、不重放或反向结束、明确未发送、无有效 XID 的 Begin 协议错误、已发送等待取消、active 归零、本地嵌套拒绝和关闭后最终值。并行计数单元测试在接近 u64 上限时验证饱和及 active 归零。

真实 HTTP 用受控屏障保持第三个 Status 在途，采集 active=1 后才关闭对端制造未知结果。最终精确检查 observed/rejected/unknown 各 1、其他状态 0、仅 32 行及 TM 关闭前后相同计数；错误 Bearer 得到 401，结果中没有访问令牌、原 XID 和 TC 响应敏感哨兵。[目标 TCP](target-green.log)与[HTTP](export-green.log)是首轮目标证据，最终源码以全工作区回归为准。

## 固定原 Java TC 和真实 MySQL

沿用已固定的 Java TC、Connector/J、MySQL 8.4.13 和原帧观察器。原管理器经 008 持久来源正常 Begin，原 Commit 或 Rollback 成功响应被丢弃；原错误保持未知，原内存恢复入口拒绝重新发送。管理器关闭后采样仍为 Begin observed=1、原 End unknown=1、其余为 0。新管理器从持久快照只查询原 XID，最终单独保留 Status observed=1，未与原管理器混合累计。

运行器逐项核对原始帧：一次 Begin 及匹配回包、一次原 End 及被丢弃的匹配成功回包、另连接一次 Status 及匹配状态响应；独立 MySQL 读取原 send_started=1、原操作和 owner_epoch=1。然后将计数快照与固定完整字段集合比较。对每份真实结果分别制造 unknown_as_observed、duplicated_end、wrong_operation、identity_label、boolean_counter 五种削弱，均被拒绝。

- [1.99 Commit](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.99-c08e857c/tm-call-metrics.json) / [原帧与验证](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.99-c08e857c/verified.json)
- [1.99 Rollback](../java-tc-changed-commit-report/run-tm_durable_rollback_loss-1.99-8764daec/tm-call-metrics.json) / [原帧与验证](../java-tc-changed-commit-report/run-tm_durable_rollback_loss-1.99-8764daec/verified.json)
- [1.85 Commit](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.85-fbff682e/tm-call-metrics.json) / [原帧与验证](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.85-fbff682e/verified.json)
- [1.85 Rollback](../java-tc-changed-commit-report/run-tm_durable_rollback_loss-1.85-c7eed5b5/tm-call-metrics.json) / [原帧与验证](../java-tc-changed-commit-report/run-tm_durable_rollback_loss-1.85-c7eed5b5/verified.json)

对应目录保留 tm-metrics-controls.json。1.85 首轮在原 TC 绑定端口时因 Address already in use 退出，尚未执行客户端调用；[失败日志](java-commit-1.85.log)及[清理记录](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.85-8a4812d9/process-cleanup.json)保留。确认终止和清理后重新运行，以上四组最终证据通过。自有 MySQL 容器均已移除。

原 TC 场景采样器、原帧和数据库联合验证，HTTP 独立用正式协议 TCP 夹具验收；本轮没有把 HTTP 采集接入原 Java TC 现场，不以两者组合推断生产采集部署已完成。

## 最终回归

[1.99 全工作区全功能](1.99-workspace.log)、[1.85 全工作区全功能](1.85-workspace.log)各 690 项通过、313 项 ignored；命令 cargo +版本 test --workspace --all-features --offline --locked。全目标严格 Clippy 覆盖 TM、observability、MySQL 及联试辅助：[1.99](1.99-clippy.log)、[1.85](1.85-clippy.log)。外部环境 ignored 用例维持独立状态，四次原 TC 场景实际通过不能替代所有其他 ignored。格式与工具 AST/证据链接校验见 format.log 和 validation.log。

尚未完成完整 TM 事务存活状态、RM 持久分支、MySQL 本地状态、重试指标、生产代理/TLS、跨重启持续计数、采集保留和告警。WP07 总项继续开放。
