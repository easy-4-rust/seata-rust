use crate::{DbError, UndoFenceError};

/// 原提交尝试恢复不能确认结果的原因；所有变体均须保留为 Unknown。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 的本地结果未知边界。
#[derive(Debug, thiserror::Error)]
pub enum MySqlCommitRecoveryError {
    /// 006 没有原 Armed 记录；不能据此推断旧写入器或旧事务未提交。
    #[error("original MySQL commit attempt record is absent")]
    MissingAttempt,
    /// 006 或 004 的读取、身份核对、期限或独立查询失败。
    #[error("MySQL commit attempt observation remains unknown: {source}")]
    Observation {
        /// 脱敏的数据库诊断。
        #[source]
        source: DbError,
    },
    /// 原 Undo 唯一键围栏未能确认结果，包括自身 COMMIT 回包丢失。
    #[error("MySQL commit attempt fence remains unknown: {source}")]
    Fence {
        /// 原围栏阶段诊断。
        #[source]
        source: UndoFenceError,
    },
}

impl MySqlCommitRecoveryError {
    /// 无参数；所有错误均代表尚未确认结果，不授权业务或 TC 请求重放。
    pub const fn is_unknown_outcome(&self) -> bool {
        true
    }
}
