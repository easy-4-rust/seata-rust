//! WP07 既有正式事件的静态模板与字段值校验，不接纳应用自定义内容。
use std::collections::BTreeMap;
use tracing::Metadata;

/// 参数为 tracing target；返回是否属于已支持的正式事件命名空间。
pub(crate) fn known_target(target: &str) -> bool {
    matches!(
        target,
        "seata_tm::rpc"
            | "seata_rm::rpc"
            | "seata_rm::callback"
            | "seata_rm::callback_write"
            | "seata_db_mysql::control"
            | "seata_db_mysql::undo"
    )
}

// 在访问任何字段值前核对完整静态字段集合；未知字段的 Debug 也不能被调用。
/// 参数为事件静态元数据；完整字段集合匹配时返回固定消息，否则拒绝访问值。
pub(crate) fn expected_message(meta: &Metadata<'_>) -> Option<&'static str> {
    let (message, names): (&str, &[&str]) = match meta.target() {
        "seata_tm::rpc" => (
            "tm_rpc_outcome",
            &["operation", "xid", "request_id", "outcome", "message"],
        ),
        "seata_rm::rpc" => (
            "rm_rpc_outcome",
            &[
                "operation",
                "xid",
                "branch_id",
                "resource_sha256",
                "request_id",
                "outcome",
                "message",
            ],
        ),
        "seata_rm::callback" => (
            "rm_callback_decision",
            &[
                "request_id",
                "connection_generation",
                "xid",
                "branch_id",
                "resource_sha256",
                "phase",
                "branch_status",
                "result_code",
                "message",
            ],
        ),
        "seata_rm::callback_write" => (
            "rm_callback_response_write",
            &[
                "request_id",
                "connection_generation",
                "response_count",
                "outcome",
                "message",
            ],
        ),
        "seata_db_mysql::control" => (
            "mysql_control_outcome",
            &[
                "operation",
                "requested_phase",
                "xid",
                "branch_id",
                "resource_sha256",
                "outcome",
                "resolution",
                "commit_attempted",
                "message",
            ],
        ),
        "seata_db_mysql::undo" if meta.fields().field("resource_digest").is_some() => (
            "mysql_undo_checksum_rejected",
            &["xid", "branch_id", "resource_digest", "outcome", "message"],
        ),
        "seata_db_mysql::undo" => (
            "mysql_undo_outcome",
            &[
                "operation",
                "xid",
                "branch_id",
                "resource_sha256",
                "outcome",
                "observation",
                "commit_attempted",
                "message",
            ],
        ),
        _ => return None,
    };
    (meta.fields().len() == names.len()
        && names.iter().all(|name| meta.fields().field(name).is_some()))
    .then_some(message)
}
fn optional(value: &str) -> &str {
    value
        .strip_prefix("Some(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(value)
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn signed_id(value: &str, branch: bool) -> bool {
    if value == "None" {
        return true;
    }
    let value = optional(value);
    if branch {
        value.parse::<i64>().is_ok_and(|n| n > 0)
    } else {
        value.parse::<i32>().is_ok()
    }
}
/// 参数为原 target、已捕获字段和固定消息；返回全部枚举、数值及摘要是否符合模板。
pub(crate) fn valid_values(target: &str, fields: &BTreeMap<String, String>, message: &str) -> bool {
    fields.get("message").is_some_and(|v| v == message)
        && fields.iter().all(|(key, value)| match key.as_str() {
            "message" => value == message,
            "xid" => !value.is_empty(),
            "request_id" => {
                if matches!(target, "seata_tm::rpc" | "seata_rm::rpc") {
                    signed_id(value, false)
                } else {
                    value.parse::<i32>().is_ok()
                }
            }
            "branch_id" => {
                if target == "seata_rm::rpc" {
                    signed_id(value, true)
                } else {
                    value.parse::<i64>().is_ok_and(|n| n > 0)
                }
            }
            "resource_digest" => digest(value),
            "resource_sha256" => {
                if target == "seata_rm::rpc" {
                    value == "None"
                        || value
                            .strip_prefix("Some(\"")
                            .and_then(|v| v.strip_suffix("\")"))
                            .is_some_and(digest)
                } else {
                    digest(value) || (value.is_empty() && target == "seata_db_mysql::undo")
                }
            }
            "connection_generation" | "response_count" => value.parse::<u64>().is_ok_and(|v| v > 0),
            "outcome" => match message {
                "tm_rpc_outcome" | "rm_rpc_outcome" => {
                    matches!(value.as_str(), "response" | "unknown" | "not_sent")
                }
                "rm_callback_response_write" => matches!(
                    value.as_str(),
                    "local_written" | "unknown" | "not_sent" | "cancelled"
                ),
                "mysql_control_outcome" | "mysql_undo_outcome" => matches!(
                    value.as_str(),
                    "completed" | "unknown_commit" | "before_commit_error" | "cancelled"
                ),
                "mysql_undo_checksum_rejected" => value == "unknown",
                _ => false,
            },
            "observation" => matches!(
                value.as_str(),
                "consumed"
                    | "fenced_inserted"
                    | "fenced_existing"
                    | "undo_present"
                    | "not_observed"
            ),
            "resolution" => matches!(value.as_str(), "applied" | "unresolved" | "not_observed"),
            "commit_attempted" => matches!(value.as_str(), "true" | "false"),
            "requested_phase" => matches!(value.as_str(), "none" | "commit" | "rollback"),
            "phase" => matches!(value.as_str(), "Commit" | "Rollback"),
            "result_code" => matches!(value.as_str(), "Success" | "Failed"),
            "branch_status" => matches!(
                value.as_str(),
                "PhaseTwoCommitted"
                    | "PhaseTwoRollbacked"
                    | "PhaseTwoCommitFailedRetryable"
                    | "PhaseTwoCommitFailedUnretryable"
                    | "PhaseTwoRollbackFailedRetryable"
                    | "PhaseTwoRollbackFailedUnretryable"
            ),
            "operation" => match target {
                "seata_tm::rpc" => {
                    matches!(value.as_str(), "Begin" | "Commit" | "Rollback" | "Status")
                }
                "seata_rm::rpc" => matches!(
                    value.as_str(),
                    "BranchRegister" | "BranchReport" | "LockQuery"
                ),
                "seata_db_mysql::control" => matches!(
                    value.as_str(),
                    "enroll" | "bind_commit" | "bind_rollback" | "commit" | "rollback" | "observe"
                ),
                "seata_db_mysql::undo" => matches!(value.as_str(), "rollback" | "fence"),
                _ => false,
            },
            _ => false,
        })
}
