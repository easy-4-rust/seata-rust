use crate::{MySqlRecoveryError, MySqlRecoveryOperation};
use seata_rm::{BranchIdentity, BranchPhase, BranchResolution};
use sha2::{Digest, Sha256};

/// 关联原分支的控制调用结果事件；不持有连接、SQL、原错误、资源明文或恢复权限。
/// 对应 Java: 无直接对象；WP07 Rust MySQL 恢复控制结构化观测扩展。
pub(crate) struct MySqlControlEvent {
    xid: String,
    branch_id: i64,
    resource_sha256: String,
    operation: MySqlRecoveryOperation,
    requested_phase: &'static str,
    commit_attempted: bool,
    finished: bool,
}
impl MySqlControlEvent {
    /// 参数为有效原身份、实际动作与请求方向；返回尚未尝试 COMMIT 的事件作用域。
    pub(crate) fn new(
        id: &BranchIdentity,
        operation: MySqlRecoveryOperation,
        phase: Option<BranchPhase>,
    ) -> Self {
        Self {
            xid: id.xid().to_owned(),
            branch_id: id.branch_id(),
            resource_sha256: format!(
                "{:x}",
                Sha256::digest(id.resource().resource_id().as_bytes())
            ),
            operation,
            requested_phase: match phase {
                Some(BranchPhase::Commit) => "commit",
                Some(BranchPhase::Rollback) => "rollback",
                None => "none",
            },
            commit_attempted: false,
            finished: false,
        }
    }
    /// 无参数；仅由原控制 COMMIT 的既有尝试边界调用，不表示字节已送达或提交已确认。
    pub(crate) fn mark_commit_attempted(&mut self) {
        self.commit_attempted = true;
    }
    /// 参数为原返回结果；无返回值，只输出原控制事实，不修改结果、权限或数据库状态。
    pub(crate) fn finish(mut self, result: &Result<BranchResolution, MySqlRecoveryError>) {
        let (outcome, resolution) = match result {
            Ok(BranchResolution::Applied) => ("completed", "applied"),
            Ok(BranchResolution::Unknown) => ("completed", "unresolved"),
            Err(error) if error.is_unknown_outcome() => ("unknown_commit", "not_observed"),
            Err(_) => ("before_commit_error", "not_observed"),
            Ok(_) => ("completed", "not_observed"),
        };
        // 先结算标记；subscriber 若 panic，销毁不再发出第二个取消事件。
        self.finished = true;
        self.emit(outcome, resolution);
    }
    fn emit(&self, outcome: &'static str, resolution: &'static str) {
        tracing::info!(target: "seata_db_mysql::control",
            operation = self.operation.label(), requested_phase = self.requested_phase,
            xid = self.xid.as_str(), branch_id = self.branch_id, resource_sha256 = self.resource_sha256.as_str(),
            outcome, resolution, commit_attempted = self.commit_attempted,
            "mysql_control_outcome");
    }
}
impl Drop for MySqlControlEvent {
    fn drop(&mut self) {
        if !self.finished {
            self.finished = true;
            self.emit("cancelled", "not_observed");
        }
    }
}

#[cfg(test)]
#[path = "my_sql_control_event_tests.rs"]
mod tests;
