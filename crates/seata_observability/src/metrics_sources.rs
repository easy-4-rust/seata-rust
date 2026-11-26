use seata_tm::{TmMetricsSampler, TmOperation};
use seata_transport::{PoolSampler, PoolSlotState};

/// 固定 TM/RM 连接与 TM 调用的只读指标来源；没有用户提供的标签键或标签值。
/// 对应 Java: TM/RM 客户端连接监控视图的组合职责。
#[derive(Default)]
pub struct MetricsSources {
    tm: Option<PoolSampler>,
    rm: Option<PoolSampler>,
    tm_calls: Option<TmMetricsSampler>,
}

impl MetricsSources {
    /// 参数分别为 TM 和 RM 的只读采样能力；缺失角色不输出指标。
    pub fn new(tm: Option<PoolSampler>, rm: Option<PoolSampler>) -> Self {
        Self {
            tm,
            rm,
            tm_calls: None,
        }
    }

    /// 参数为 TM 所有者签发的只读调用采样器；返回增加固定 TM 调用指标的新来源。
    /// 不接受自定义标签或事务身份；也可只提供此采样器而不导出连接指标。
    ///
    ///     use seata_observability::MetricsSources;
    ///     use seata_tm::TransactionManager;
    ///     fn sources(tm: &TransactionManager) -> MetricsSources {
    ///         MetricsSources::new(Some(tm.connection_sampler()), None)
    ///             .with_tm_calls(tm.metrics_sampler())
    ///     }
    pub fn with_tm_calls(mut self, source: TmMetricsSampler) -> Self {
        self.tm_calls = Some(source);
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.tm.is_none() && self.rm.is_none() && self.tm_calls.is_none()
    }

    pub(crate) fn render(&self) -> String {
        let mut output = String::new();
        for (role, source) in [("tm", &self.tm), ("rm", &self.rm)] {
            let Some(source) = source else {
                continue;
            };
            let snapshot = source.snapshot();
            let closed = u8::from(snapshot.closed);
            output.push_str(&format!(
                "seata_transport_pool_closed{{role=\"{role}\"}} {closed}\n"
            ));
            for (index, slot) in snapshot.slots.iter().enumerate() {
                let state = match slot.state {
                    PoolSlotState::Connecting => "connecting",
                    PoolSlotState::Connected => "connected",
                    PoolSlotState::Registering => "registering",
                    PoolSlotState::Ready => "ready",
                    PoolSlotState::Degraded => "degraded",
                    PoolSlotState::Backoff => "backoff",
                    PoolSlotState::Stopped => "stopped",
                };
                output.push_str(&format!(
                    "seata_transport_slot_state{{role=\"{role}\",slot=\"{index}\",state=\"{state}\"}} 1\n"
                ));
                let pending = slot.connection.as_ref().map_or(0, |value| value.pending);
                output.push_str(&format!(
                    "seata_transport_pending_requests{{role=\"{role}\",slot=\"{index}\"}} {pending}\n"
                ));
            }
            let totals = snapshot.cumulative;
            for (name, value) in [
                ("write_attempted", totals.write_attempted),
                ("response_write_attempted", totals.response_write_attempted),
                ("responses_written", totals.responses_written),
                ("completed", totals.completed),
                ("timed_out_before_send", totals.timed_out_before_send),
                ("timed_out_after_send", totals.timed_out_after_send),
                ("cancelled_before_send", totals.cancelled_before_send),
                ("cancelled_after_send", totals.cancelled_after_send),
                ("unmatched_responses", totals.unmatched_responses),
                ("heartbeats_received", totals.heartbeats_received),
                ("heartbeat_requests_sent", totals.heartbeat_requests_sent),
                ("heartbeat_responses_sent", totals.heartbeat_responses_sent),
                ("read_idle_expirations", totals.read_idle_expirations),
                ("inbound_requests", totals.inbound_requests),
            ] {
                output.push_str(&format!(
                    "seata_transport_{name}_total{{role=\"{role}\"}} {value}\n"
                ));
            }
        }
        if let Some(source) = &self.tm_calls {
            // 操作和结果只能来自固定枚举，避免把业务身份变成高基数标签。
            for (operation, name) in [
                (TmOperation::Begin, "begin"),
                (TmOperation::Commit, "commit"),
                (TmOperation::Rollback, "rollback"),
                (TmOperation::Status, "status"),
            ] {
                let counts = source.snapshot(operation);
                output.push_str(&format!(
                    "seata_tm_calls_active{{operation=\"{name}\"}} {}\n",
                    counts.active
                ));
                for (outcome, count) in [
                    ("observed", counts.observed),
                    ("tc_rejected", counts.tc_rejected),
                    ("unknown", counts.unknown),
                    ("not_sent", counts.not_sent),
                    ("local_rejected", counts.local_rejected),
                    ("cancelled", counts.cancelled),
                ] {
                    output.push_str(&format!(
                        "seata_tm_calls_total{{operation=\"{name}\",outcome=\"{outcome}\"}} {count}\n"));
                }
                output.push_str(&format!(
                    "seata_tm_protocol_errors_total{{operation=\"{name}\"}} {}\n",
                    counts.protocol_errors
                ));
            }
        }
        output
    }
}
