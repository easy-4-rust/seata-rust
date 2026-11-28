use crate::{UndoFenceError, UndoFenceObservation, UndoRollbackError, UndoRollbackObservation};
use sha2::{Digest, Sha256};

/// 独立 Undo 调用的单次只读事件作用域，不持有数据库连接、原错误或恢复权限。
/// 对应 Java: 无直接对象；WP07 Rust 独立 Undo 结果观测扩展。
pub(crate) struct MySqlUndoEvent {
    xid: String,
    branch_id: i64,
    resource_sha256: String,
    operation: &'static str,
    commit_attempted: bool,
    finished: bool,
}
impl MySqlUndoEvent {
    /// 参数为原身份、规范资源标识及固定动作；非法身份返回 None，不输出未经验证的身份。
    pub(crate) fn new(
        xid: &str,
        branch_id: i64,
        resource: Option<&str>,
        operation: &'static str,
    ) -> Option<Self> {
        if xid.is_empty() || xid.len() > 128 || branch_id <= 0 {
            return None;
        }
        Some(Self {
            xid: xid.into(),
            branch_id,
            resource_sha256: resource
                .map(|r| format!("{:x}", Sha256::digest(r.as_bytes())))
                .unwrap_or_default(),
            operation,
            commit_attempted: false,
            finished: false,
        })
    }
    /// 无参数；在原调用可能发送控制 COMMIT 的既有边界标记，不表示发送或提交确认。
    pub(crate) fn mark_commit_attempted(&mut self) {
        self.commit_attempted = true;
    }
    /// 参数为原补偿返回结果；只记录原结果，不推断未知提交的服务端效果。
    pub(crate) fn finish_rollback(
        self,
        result: &Result<UndoRollbackObservation, UndoRollbackError>,
    ) {
        let (outcome, observation) = match result {
            Ok(UndoRollbackObservation::Consumed { .. }) => ("completed", "consumed"),
            Ok(UndoRollbackObservation::Fenced { inserted: true }) => {
                ("completed", "fenced_inserted")
            }
            Ok(UndoRollbackObservation::Fenced { inserted: false }) => {
                ("completed", "fenced_existing")
            }
            Err(e) if e.is_unknown_outcome() => ("unknown_commit", "not_observed"),
            Err(_) => ("before_commit_error", "not_observed"),
        };
        self.finish(outcome, observation);
    }
    /// 参数为原围栏返回结果；既有围栏和 Normal 存在只表示本次观察。
    pub(crate) fn finish_fence(self, result: &Result<UndoFenceObservation, UndoFenceError>) {
        let (outcome, observation) = match result {
            Ok(UndoFenceObservation::UndoPresent) => ("completed", "undo_present"),
            Ok(UndoFenceObservation::Fenced { inserted: true }) => ("completed", "fenced_inserted"),
            Ok(UndoFenceObservation::Fenced { inserted: false }) => {
                ("completed", "fenced_existing")
            }
            Err(e) if e.is_unknown_outcome() => ("unknown_commit", "not_observed"),
            Err(_) => ("before_commit_error", "not_observed"),
        };
        self.finish(outcome, observation);
    }
    fn finish(mut self, outcome: &'static str, observation: &'static str) {
        // 先结算；subscriber panic 解栈时不会再误发一次 cancelled。
        self.finished = true;
        self.emit(outcome, observation);
    }
    fn emit(&self, outcome: &'static str, observation: &'static str) {
        tracing::info!(target:"seata_db_mysql::undo",operation=self.operation,xid=self.xid.as_str(),branch_id=self.branch_id,resource_sha256=self.resource_sha256.as_str(),outcome,observation,commit_attempted=self.commit_attempted,"mysql_undo_outcome");
    }
}
impl Drop for MySqlUndoEvent {
    fn drop(&mut self) {
        if !self.finished {
            self.finished = true;
            self.emit("cancelled", "not_observed");
        }
    }
}

#[cfg(test)]
#[path = "my_sql_undo_event_tests.rs"]
mod tests;
