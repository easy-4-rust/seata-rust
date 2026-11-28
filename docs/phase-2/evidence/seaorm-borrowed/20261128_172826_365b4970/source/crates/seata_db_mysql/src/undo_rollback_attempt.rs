use crate::{DbError, UndoRollbackError, UndoRollbackObservation};
use std::future::Future;

/// 保留控制事务已观察的首个失败及 COMMIT 发送边界。
/// 对应 Java: AbstractUndoLogManager#undo 在清理失败后保留原异常的行为。
#[derive(Default)]
pub(crate) struct UndoRollbackAttempt {
    failure: Option<DbError>,
    commit_attempted: bool,
}

impl UndoRollbackAttempt {
    /// 参数为已观察的原错误及独占连接的回滚 Future；先保存原错误，再尝试清理。
    pub(crate) async fn abort(
        &mut self,
        source: DbError,
        cleanup: impl Future<Output = Result<(), sqlx::Error>>,
    ) {
        self.failure = Some(source);
        // 调用方把获取的池连接标为 close_on_drop，清理失败或取消也不会将其归还。
        let _ = cleanup.await;
    }

    /// 无参数；取出先前保留的原始失败，供托管 RM 按分支身份分类。
    pub(crate) fn into_failure(self) -> Option<DbError> {
        self.failure
    }

    /// 无参数；进入可能发送控制 COMMIT 的阶段。
    pub(crate) fn start_commit(&mut self) {
        self.commit_attempted = true;
    }

    /// 参数为原事务身份、在预算内取得的结果及最终期限检查，返回公开阶段结果。
    pub(crate) fn finish(
        self,
        xid: &str,
        branch_id: i64,
        ready: Option<Result<Option<UndoRollbackObservation>, DbError>>,
        expired: bool,
    ) -> Result<UndoRollbackObservation, UndoRollbackError> {
        let source = match (self.failure, ready) {
            (Some(source), _) => source,
            (None, Some(Err(source))) if !self.commit_attempted || !expired => source,
            (None, Some(Ok(Some(observation)))) if !expired => return Ok(observation),
            _ => DbError::Deadline {
                operation: "Undo rollback",
            },
        };
        let xid = xid.to_owned();
        Err(if self.commit_attempted {
            UndoRollbackError::CommitOutcomeUnknown {
                xid,
                branch_id,
                source,
            }
        } else {
            UndoRollbackError::BeforeCommit {
                xid,
                branch_id,
                source,
            }
        })
    }
}
