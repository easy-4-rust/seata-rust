# seata-observability

本 crate 提供本机拉取式连接和 TM 调用指标服务。TM 和 RM 管理器通过 `connection_sampler()` 签发只读采样能力，导出器不持有业务调用或关闭连接池的能力。

```rust,ignore
use seata_observability::{MetricsExporter, MetricsServerConfig, MetricsSources};

let config = MetricsServerConfig::new(
    "127.0.0.1:9464".parse()?,
    metrics_token_from_secret_store,
)?;
let sources = MetricsSources::new(
    Some(transaction_manager.connection_sampler()),
    Some(resource_manager.connection_sampler()),
).with_tm_calls(transaction_manager.metrics_sampler());
let exporter = MetricsExporter::start(config, sources).await?;
// 进程关闭时调用 exporter.shutdown().await?。
```

令牌须为 32–128 字节可见 ASCII；服务拒绝非回环地址和空采样源。采集请求为 `GET /metrics`，必须带唯一的 `Authorization: Bearer <token>` 请求头。服务限制并发采集数、请求头长度和读取时长，关闭后停止监听并取消在途采集。远程采集须经部署侧的认证和 TLS 代理。

指标仅使用固定 `tm`/`rm` 角色、配置槽位编号和固定状态。`seata_transport_pool_closed`、`seata_transport_slot_state`、`seata_transport_pending_requests` 为当前 gauge；其余 `seata_transport_*_total` 为池进程生命周期 counter。端点不导出 XID、BranchId、资源 URL、请求 ID、SQL、锁键或凭据。池重建或进程重启会重置 counter；采集端须按进程生命周期处理。

TmMetricsSampler 是管理器签发的只读调用能力，with_tm_calls 可独立提供 TM 调用指标或与原连接指标组合。seata_tm_calls_active 按四种固定 operation 统计当前原生调用；seata_tm_calls_total 的固定 outcome 为 observed、tc_rejected、unknown、not_sent、local_rejected、cancelled；seata_tm_protocol_errors_total 是可与结果重叠的协议错误计数。新增指标共 32 条固定序列，不使用业务身份作标签。

observed 只表示有效权威成功响应，包含 AsyncCommitting、Rollbacking 等非最终状态。取消不推断写入边界，仍需结合原传输和持久恢复事实。管理器关闭后可采样最终累计值；重建管理器或进程重启归零。快照逐字段读取，不保证同一瞬间的一致总数，累计饱和不回绕。

[双版本 TCP、HTTP、原 Java TC 故障路径及反例证据](../../docs/phase-2/evidence/tm-call-metrics/README.md)。完整 TM 事务存活状态、RM 持久分支状态、MySQL 本地结果、生产代理/权限、告警及故障演练仍需单独验收，不能据此关闭 WP07 门禁。


RM 分支来源通过 with_rm_branches(resource_manager.metrics_sampler()) 或 FileBranchStore 的同名采样器接入，也可单独作为来源。固定 34 条序列包括两种 phase 下的四种保留日志 state、dispatch active、九种分派 outcome、两种资源调用 operation，以及 journal_faulted/journal_closed。所有标签来自固定枚举，不导出分支身份、文件路径或凭据。

日志 gauge 从同步索引更新并在重开时重建；counter 属于一次日志打开周期。采样器保留不会延长日志文件锁寿命。RM 关闭后外部仍持有日志时，journal_closed 仍为 0。详见 [双版本原连接回调、HTTP 与原 Java TC 证据](../../docs/phase-2/evidence/rm-branch-metrics/README.md)。RM 出站调用结果、MySQL 本地状态和生产采集/告警仍待验收。


RM 出站来源通过 with_rm_calls(resource_manager.call_metrics_sampler()) 或客户端同名方法接入，可独立使用。固定 24 条序列为三种 operation 的 calls_active、六种 outcome 的 calls_total 和 protocol_errors_total；不包含业务身份。注册回执只计一次，LockQuery false 仍是 observed，取消不推断发送边界。该来源与日志作用域的 with_rm_branches 独立；管理器关闭后保留客户端的本地调用仍可能增加计数。见 [双版本 TCP、HTTP 和原 Java TC 证据](../../docs/phase-2/evidence/rm-call-metrics/README.md)。MySQL 本地状态、生产采集、告警和完整 WP07 仍开放。


显式 mysql Cargo 特性增加 with_mysql_recovery(source.recovery_metrics_sampler())，也接受管理器同名只读能力；默认关闭，默认运行依赖中没有 MySQL/SQLx。六种固定控制动作各导出 active、四种互斥 outcome 及两种成功 resolution，共 42 条序列。completed 仅表示控制事务返回 Ok；Applied 包含读取已有终态，不能推断实际业务效果次数。取消与 COMMIT 确认未知分开，不输出业务身份。见 [MySQL 控制指标证据](../../docs/phase-2/evidence/mysql-control-metrics/README.md)。
