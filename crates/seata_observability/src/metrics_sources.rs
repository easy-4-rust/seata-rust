#[cfg(feature = "mysql")]
use seata_db_mysql::{
    MySqlRecoveryMetricsSampler, MySqlRecoveryOperation, MySqlUndoMetricsSampler,
    MySqlUndoOperation,
};
use seata_rm::{BranchPhase, RmCallMetricsSampler, RmMetricsSampler, RmOperation};
use seata_tm::{TmMetricsSampler, TmOperation};
use seata_transport::{PoolSampler, PoolSlotState};

/// 固定 TM/RM 连接、调用、分支与可选 MySQL 控制事务的只读指标来源；没有用户提供的标签键或标签值。
/// 对应 Java: TM/RM 客户端连接监控视图的组合职责。
#[derive(Default)]
pub struct MetricsSources {
    tm: Option<PoolSampler>,
    rm: Option<PoolSampler>,
    tm_calls: Option<TmMetricsSampler>,
    rm_branches: Option<RmMetricsSampler>,
    rm_calls: Option<RmCallMetricsSampler>,
    #[cfg(feature = "mysql")]
    mysql_recovery: Option<MySqlRecoveryMetricsSampler>,
    #[cfg(feature = "mysql")]
    mysql_undo: Option<MySqlUndoMetricsSampler>,
    #[cfg(feature = "mysql")]
    mysql_phase_one: Option<seata_db_mysql::MySqlPhaseOneMetricsSampler>,
}

impl MetricsSources {
    /// 参数分别为 TM 和 RM 的只读采样能力；缺失角色不输出指标。
    pub fn new(tm: Option<PoolSampler>, rm: Option<PoolSampler>) -> Self {
        Self {
            tm,
            rm,
            tm_calls: None,
            rm_branches: None,
            rm_calls: None,
            #[cfg(feature = "mysql")]
            mysql_recovery: None,
            #[cfg(feature = "mysql")]
            mysql_undo: None,
            #[cfg(feature = "mysql")]
            mysql_phase_one: None,
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

    /// 参数为同一日志打开周期的只读 RM 分支采样器；返回增加固定分支指标的新来源。
    /// 采样器不持有文件或分派能力；也可单独作为采样来源。
    ///
    ///     use seata_observability::MetricsSources;
    ///     use seata_rm::ResourceManager;
    ///     fn sources(rm: &ResourceManager) -> MetricsSources {
    ///         MetricsSources::new(None, Some(rm.connection_sampler()))
    ///             .with_rm_branches(rm.metrics_sampler())
    ///     }
    pub fn with_rm_branches(mut self, source: RmMetricsSampler) -> Self {
        self.rm_branches = Some(source);
        self
    }

    /// 参数为当前 RM 管理器及客户端克隆的只读调用源；返回增加固定调用指标的新来源。
    /// 源不持有池或资源权限，既可单独提供，也可与连接及分支指标组合。
    ///
    ///     use seata_observability::MetricsSources;
    ///     use seata_rm::ResourceClient;
    ///     fn sources(client: &ResourceClient) -> MetricsSources {
    ///         MetricsSources::default().with_rm_calls(client.call_metrics_sampler())
    ///     }
    pub fn with_rm_calls(mut self, source: RmCallMetricsSampler) -> Self {
        self.rm_calls = Some(source);
        self
    }

    /// 参数为数据源签发的只读恢复控制计数；返回增加 42 条固定序列的新来源。
    /// 不授予连接、恢复或关闭权限，也可独立提供，不输出业务身份。
    #[cfg(feature = "mysql")]
    pub fn with_mysql_recovery(mut self, source: MySqlRecoveryMetricsSampler) -> Self {
        self.mysql_recovery = Some(source);
        self
    }

    /// 参数为数据源签发的只读独立 Undo 计数；返回增加 20 条固定序列的新来源。
    /// 不授予数据库权限，不输出业务身份；此计数不与恢复管理器控制次数相加推断效果。
    #[cfg(feature = "mysql")]
    pub fn with_mysql_undo(mut self, source: MySqlUndoMetricsSampler) -> Self {
        self.mysql_undo = Some(source);
        self
    }

    /// 参数为数据源签发的一阶段只读计数；返回增加 31 条固定序列的来源。
    /// 不授权提交或报告；各阶段观察不可相加当作业务效果次数。
    #[cfg(feature = "mysql")]
    pub fn with_mysql_phase_one(
        mut self,
        source: seata_db_mysql::MySqlPhaseOneMetricsSampler,
    ) -> Self {
        self.mysql_phase_one = Some(source);
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        #[cfg(feature = "mysql")]
        if self.mysql_recovery.is_some()
            || self.mysql_undo.is_some()
            || self.mysql_phase_one.is_some()
        {
            return false;
        }
        self.tm.is_none()
            && self.rm.is_none()
            && self.tm_calls.is_none()
            && self.rm_branches.is_none()
            && self.rm_calls.is_none()
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
        if let Some(source) = &self.rm_branches {
            let lifecycle = source.snapshot(BranchPhase::Commit);
            output.push_str(&format!(
                "seata_rm_journal_faulted {}\n",
                u8::from(lifecycle.journal_faulted)
            ));
            output.push_str(&format!(
                "seata_rm_journal_closed {}\n",
                u8::from(lifecycle.journal_closed)
            ));
            for (phase, name) in [
                (BranchPhase::Commit, "commit"),
                (BranchPhase::Rollback, "rollback"),
            ] {
                let counts = source.snapshot(phase);
                for (state, count) in [
                    ("pending", counts.journal_pending),
                    ("completed", counts.journal_completed),
                    ("retry_safe", counts.journal_retry_safe),
                    ("rejected", counts.journal_rejected),
                ] {
                    output.push_str(&format!(
                        "seata_rm_journal_branches{{phase=\"{name}\",state=\"{state}\"}} {count}\n"
                    ));
                }
                output.push_str(&format!(
                    "seata_rm_dispatch_active{{phase=\"{name}\"}} {}\n",
                    counts.active
                ));
                for (outcome, count) in [
                    ("applied", counts.applied),
                    ("rejected", counts.rejected),
                    ("retry_safe", counts.retry_safe),
                    ("unknown", counts.unknown),
                    ("manual_intervention", counts.manual_intervention),
                    ("duplicate_completed", counts.duplicate_completed),
                    ("duplicate_rejected", counts.duplicate_rejected),
                    ("error", counts.errors),
                    ("cancelled", counts.cancelled),
                ] {
                    output.push_str(&format!("seata_rm_dispatch_total{{phase=\"{name}\",outcome=\"{outcome}\"}} {count}\n"));
                }
                for (operation, count) in [
                    ("apply", counts.apply_attempts),
                    ("recover", counts.recover_attempts),
                ] {
                    output.push_str(&format!("seata_rm_resource_attempts_total{{phase=\"{name}\",operation=\"{operation}\"}} {count}\n"));
                }
            }
        }
        if let Some(source) = &self.rm_calls {
            for (operation, name) in [
                (RmOperation::BranchRegister, "branch_register"),
                (RmOperation::BranchReport, "branch_report"),
                (RmOperation::LockQuery, "lock_query"),
            ] {
                let counts = source.snapshot(operation);
                output.push_str(&format!(
                    "seata_rm_calls_active{{operation=\"{name}\"}} {}\n",
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
                    output.push_str(&format!("seata_rm_calls_total{{operation=\"{name}\",outcome=\"{outcome}\"}} {count}\n"));
                }
                output.push_str(&format!(
                    "seata_rm_protocol_errors_total{{operation=\"{name}\"}} {}\n",
                    counts.protocol_errors
                ));
            }
        }
        #[cfg(feature = "mysql")]
        if let Some(source) = &self.mysql_recovery {
            for (operation, name) in [
                (MySqlRecoveryOperation::Enroll, "enroll"),
                (MySqlRecoveryOperation::Observe, "observe"),
                (MySqlRecoveryOperation::BindCommit, "bind_commit"),
                (MySqlRecoveryOperation::BindRollback, "bind_rollback"),
                (MySqlRecoveryOperation::Commit, "commit"),
                (MySqlRecoveryOperation::Rollback, "rollback"),
            ] {
                let c = source.snapshot(operation);
                output.push_str(&format!(
                    "seata_mysql_control_active{{operation=\"{name}\"}} {}\n",
                    c.active
                ));
                for (outcome, count) in [
                    ("completed", c.completed),
                    ("unknown_commit", c.unknown_commit),
                    ("before_commit_error", c.before_commit_error),
                    ("cancelled", c.cancelled),
                ] {
                    output.push_str(&format!("seata_mysql_control_total{{operation=\"{name}\",outcome=\"{outcome}\"}} {count}\n"));
                }
                for (resolution, count) in [("applied", c.applied), ("unresolved", c.unresolved)] {
                    output.push_str(&format!("seata_mysql_control_resolutions_total{{operation=\"{name}\",resolution=\"{resolution}\"}} {count}\n"));
                }
            }
        }
        #[cfg(feature = "mysql")]
        if let Some(source) = &self.mysql_undo {
            for (operation, name) in [
                (MySqlUndoOperation::Rollback, "rollback"),
                (MySqlUndoOperation::Fence, "fence"),
            ] {
                let c = source.snapshot(operation);
                output.push_str(&format!(
                    "seata_mysql_undo_active{{operation=\"{name}\"}} {}\n",
                    c.active
                ));
                for (outcome, count) in [
                    ("completed", c.completed),
                    ("unknown_commit", c.unknown_commit),
                    ("before_commit_error", c.before_commit_error),
                    ("cancelled", c.cancelled),
                ] {
                    output.push_str(&format!("seata_mysql_undo_total{{operation=\"{name}\",outcome=\"{outcome}\"}} {count}\n"));
                }
                for (observation, count) in [
                    ("consumed", c.consumed),
                    ("fenced_inserted", c.fenced_inserted),
                    ("fenced_existing", c.fenced_existing),
                    ("undo_present", c.undo_present),
                ] {
                    output.push_str(&format!("seata_mysql_undo_observations_total{{operation=\"{name}\",observation=\"{observation}\"}} {count}\n"));
                }
                output.push_str(&format!(
                    "seata_mysql_undo_restored_rows_observed_total{{operation=\"{name}\"}} {}\n",
                    c.observed_restored_rows
                ));
            }
        }
        #[cfg(feature = "mysql")]
        if let Some(source) = &self.mysql_phase_one {
            for operation in seata_db_mysql::MySqlPhaseOneOperation::ALL {
                let name = operation.label();
                let counts = source.snapshot(operation);
                output.push_str(&format!(
                    "seata_mysql_phase_one_active{{operation=\"{name}\"}} {}\n",
                    counts.active
                ));
                for outcome in operation.outcomes() {
                    output.push_str(&format!(
                        "seata_mysql_phase_one_total{{operation=\"{name}\",outcome=\"{}\"}} {}\n",
                        outcome.label(),
                        counts.count(*outcome)
                    ));
                }
            }
        }
        output
    }
}
