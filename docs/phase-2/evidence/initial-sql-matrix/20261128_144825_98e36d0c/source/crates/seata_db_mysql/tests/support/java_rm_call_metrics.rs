//! 固定原 TC 出站调用指标的固定字段保存，不导出身份或原请求。
use crate::fixture;
use seata_rm::{RmCallMetricsSampler, RmOperation};

/// 参数为只读采样器及受控输出名；保存三种固定操作，不授予发送能力。
pub(super) fn save(source: &RmCallMetricsSampler, name: &str) {
    let mut values = serde_json::Map::new();
    for (operation, name) in [
        (RmOperation::BranchRegister, "branch_register"),
        (RmOperation::BranchReport, "branch_report"),
        (RmOperation::LockQuery, "lock_query"),
    ] {
        let counts = source.snapshot(operation);
        values.insert(name.into(),serde_json::json!({
            "active":counts.active,"observed":counts.observed,"tc_rejected":counts.tc_rejected,
            "unknown":counts.unknown,"not_sent":counts.not_sent,"local_rejected":counts.local_rejected,
            "cancelled":counts.cancelled,"protocol_errors":counts.protocol_errors,
        }));
    }
    fixture::save(name, &serde_json::Value::Object(values));
}
