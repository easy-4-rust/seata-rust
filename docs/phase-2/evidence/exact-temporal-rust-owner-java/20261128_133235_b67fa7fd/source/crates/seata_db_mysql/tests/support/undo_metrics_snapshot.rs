//! 独立 Undo 计数的测试 JSON 快照，不向生产采样器引入序列化依赖。
use seata_db_mysql::MySqlUndoMetrics;
pub fn snapshot(c: MySqlUndoMetrics) -> serde_json::Value {
    serde_json::json!({"active":c.active,"completed":c.completed,"unknown_commit":c.unknown_commit,
        "before_commit_error":c.before_commit_error,"cancelled":c.cancelled,"consumed":c.consumed,
        "fenced_inserted":c.fenced_inserted,"fenced_existing":c.fenced_existing,
        "undo_present":c.undo_present,"observed_restored_rows":c.observed_restored_rows})
}
