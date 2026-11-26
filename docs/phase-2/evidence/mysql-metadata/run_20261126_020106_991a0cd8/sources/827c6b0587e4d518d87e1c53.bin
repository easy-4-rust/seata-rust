# seata-observability

本 crate 提供本机拉取式连接指标服务。TM 和 RM 管理器通过 `connection_sampler()` 签发只读采样能力，导出器不持有业务调用或关闭连接池的能力。

```rust,ignore
use seata_observability::{MetricsExporter, MetricsServerConfig, MetricsSources};

let config = MetricsServerConfig::new(
    "127.0.0.1:9464".parse()?,
    metrics_token_from_secret_store,
)?;
let sources = MetricsSources::new(
    Some(transaction_manager.connection_sampler()),
    Some(resource_manager.connection_sampler()),
);
let exporter = MetricsExporter::start(config, sources).await?;
// 进程关闭时调用 exporter.shutdown().await?。
```

令牌须为 32–128 字节可见 ASCII；服务拒绝非回环地址和空采样源。采集请求为 `GET /metrics`，必须带唯一的 `Authorization: Bearer <token>` 请求头。服务限制并发采集数、请求头长度和读取时长，关闭后停止监听并取消在途采集。远程采集须经部署侧的认证和 TLS 代理。

指标仅使用固定 `tm`/`rm` 角色、配置槽位编号和固定状态。`seata_transport_pool_closed`、`seata_transport_slot_state`、`seata_transport_pending_requests` 为当前 gauge；其余 `seata_transport_*_total` 为池进程生命周期 counter。端点不导出 XID、BranchId、资源 URL、请求 ID、SQL、锁键或凭据。池重建或进程重启会重置 counter；采集端须按进程生命周期处理。

该服务只覆盖连接层。TM/RM 业务结果、持久分支状态、MySQL 本地结果、生产代理/权限、告警及故障演练仍需单独验收，不能据此关闭 WP07 门禁。
