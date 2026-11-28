//! 原 Java TC/MySQL 控制事务指标，保留固定字段而不导出身份。
use crate::fixture;
use seata_db_mysql::{MySqlRecoveryMetricsSampler, MySqlRecoveryOperation};
/// 参数为只读源；保存六种控制动作的完整固定快照。
pub(super) fn save(source: &MySqlRecoveryMetricsSampler) {
    let mut values = serde_json::Map::new();
    for (operation, name) in [
        (MySqlRecoveryOperation::Enroll, "enroll"),
        (MySqlRecoveryOperation::Observe, "observe"),
        (MySqlRecoveryOperation::BindCommit, "bind_commit"),
        (MySqlRecoveryOperation::BindRollback, "bind_rollback"),
        (MySqlRecoveryOperation::Commit, "commit"),
        (MySqlRecoveryOperation::Rollback, "rollback"),
    ] {
        let c = source.snapshot(operation);
        values.insert(name.into(),serde_json::json!({"active":c.active,"completed":c.completed,"unknown_commit":c.unknown_commit,"before_commit_error":c.before_commit_error,"cancelled":c.cancelled,"applied":c.applied,"unresolved":c.unresolved}));
    }
    fixture::save(
        "mysql-control-metrics.json",
        &serde_json::Value::Object(values),
    );
}
