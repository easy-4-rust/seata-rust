use crate::DbError;
use std::fmt;

/// 原Undo补偿调用的阶段错误；不以未提交当前控制事务推断旧业务未生效。
/// 对应 Java: AbstractUndoLogManager#undo的SQLException边界及Rust显式未知提交扩展。
#[derive(thiserror::Error)]
pub enum UndoRollbackError {
    /// 本地原身份不合法，尚未进行补偿SQL。
    #[error("invalid Undo rollback identity")]
    InvalidIdentity,
    /// 本次补偿尚未尝试控制COMMIT；旧业务/旧控制尝试的状态仍未知。
    #[error("Undo rollback failed before control COMMIT: {source}")]
    BeforeCommit {
        /// 原样保留的有界XID，不在Debug展示。
        xid: String,
        /// 原分支号。
        branch_id: i64,
        /// 脱敏的数据库阶段错误。
        source: DbError,
    },
    /// 控制COMMIT已经可能发送，没有权威完成结果；不可返回Fenced或自动重发。
    #[error("Undo rollback control COMMIT outcome is unknown: {source}")]
    CommitOutcomeUnknown {
        /// 原样保留的有界XID，不在Debug展示。
        xid: String,
        /// 原分支号。
        branch_id: i64,
        /// 脱敏的数据库阶段错误。
        source: DbError,
    },
}
impl UndoRollbackError {
    /// 无参数；返回本次控制COMMIT是否已可能发送而没有权威结果，不授权业务或控制重放。
    pub const fn is_unknown_outcome(&self) -> bool {
        matches!(self, Self::CommitOutcomeUnknown { .. })
    }
    /// 无参数；已验证原身份返回XID/分支号，输入非法时返回None。
    pub fn identity(&self) -> Option<(&str, i64)> {
        match self {
            Self::BeforeCommit { xid, branch_id, .. }
            | Self::CommitOutcomeUnknown { xid, branch_id, .. } => Some((xid, *branch_id)),
            Self::InvalidIdentity => None,
        }
    }
}
impl fmt::Debug for UndoRollbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentity => f.write_str("InvalidIdentity"),
            Self::BeforeCommit {
                xid,
                branch_id,
                source,
            }
            | Self::CommitOutcomeUnknown {
                xid,
                branch_id,
                source,
            } => f
                .debug_struct(if self.is_unknown_outcome() {
                    "CommitOutcomeUnknown"
                } else {
                    "BeforeCommit"
                })
                .field("xid_bytes", &xid.len())
                .field("branch_id", branch_id)
                .field("source", source)
                .finish(),
        }
    }
}
