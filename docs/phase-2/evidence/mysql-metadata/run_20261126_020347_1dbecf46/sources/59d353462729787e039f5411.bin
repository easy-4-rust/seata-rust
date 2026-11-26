use crate::{TmError, TransactionCompletion, Xid};
use thiserror::Error;
/// 业务作用域错误，保留业务失败与回滚失败两份信息，异步Drop不伪造恢复结果。
/// 对应 Java: org.apache.seata.tm.api.transaction.TransactionExecutionException
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransactionError<E> {
    /// Begin/Commit/传播检查等TM失败；未知Commit不进入业务回滚分支。
    #[error("全局事务调用失败：{0}")]
    Tm(#[source] Box<TmError>),
    /// 业务失败；仅Launcher尝试一次回滚，参与方不产生全局二阶段。
    #[error("业务执行失败；回滚结果单独保留")]
    Business {
        /// 原业务错误，不被回滚错误覆盖。
        error: E,
        /// 原XID，无事务路径为None。
        xid: Option<Xid>,
        /// Launcher的实际回滚结果；参与/无事务路径为None。
        rollback: Option<Result<TransactionCompletion, Box<TmError>>>,
    },
    /// 本地业务事务期限届满；不是传输超时或明确TC拒绝。
    #[error("全局业务作用域期限已到；恢复结果单独保留")]
    Deadline {
        /// 原XID，可用于后续恢复。
        xid: Xid,
        /// Launcher实际一次回滚；Participant标记表示没有全局回滚。
        rollback: Result<TransactionCompletion, Box<TmError>>,
    },
}
impl<E> From<TmError> for TransactionError<E> {
    fn from(value: TmError) -> Self {
        Self::Tm(Box::new(value))
    }
}
