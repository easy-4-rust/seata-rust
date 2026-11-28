//! 一阶段指标的测试封存，不把业务身份加入产品指标。
use crate::fixture;
use seata_db_mysql::{MySqlPhaseOneMetricsSampler, MySqlPhaseOneOperation};
use std::sync::atomic::{AtomicU64, Ordering};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub fn value(source: &MySqlPhaseOneMetricsSampler) -> serde_json::Value {
    let mut stages = serde_json::Map::new();
    for operation in MySqlPhaseOneOperation::ALL {
        let counts = source.snapshot(operation);
        let outcomes: serde_json::Map<String, serde_json::Value> = operation
            .outcomes()
            .iter()
            .map(|outcome| (outcome.label().into(), counts.count(*outcome).into()))
            .collect();
        stages.insert(
            operation.label().into(),
            serde_json::json!({"active":counts.active,"outcomes":outcomes}),
        );
    }
    serde_json::Value::Object(stages)
}
pub fn save(source: &MySqlPhaseOneMetricsSampler) {
    if std::env::var_os("SEATA_UNDO_STORE_OUT").is_none() {
        return;
    }
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    fixture::save(
        &format!("phase-one-stage-{}-{sequence}.json", std::process::id()),
        &value(source),
    );
}
