use crate::DbError;
use seata_rm::BranchIdentity;

/// 独立恢复控制事务的阶段错误；原身份可读取，诊断沿用身份与数据库脱敏。
/// 对应 Java: TransactionException；Rust 显式区分登记/恢复提交前失败与确认未知。
#[derive(Debug, thiserror::Error)]
pub enum MySqlRecoveryError {
    /// 尚未开始数据库控制操作的身份校验失败。
    #[error("invalid MySQL RM recovery identity")]
    InvalidIdentity,
    /// 本次控制 COMMIT 尚未开始；不推断其他实例没有执行。
    #[error("MySQL RM recovery failed before control commit: {source}")]
    BeforeCommit {
        /// 完整原始分支身份。
        identity: Box<BranchIdentity>,
        /// 脱敏的具体数据库原因。
        source: DbError,
    },
    /// 已开始控制 COMMIT，缺少期限内有效确认；必须按原身份恢复。
    #[error("MySQL RM recovery control commit outcome unknown: {source}")]
    CommitOutcomeUnknown {
        /// 完整原始分支身份。
        identity: Box<BranchIdentity>,
        /// 脱敏的具体数据库原因。
        source: DbError,
    },
}
impl MySqlRecoveryError {
    /// 无参数；返回错误关联的原身份借用，本地非法输入没有可接受身份。
    pub fn identity(&self) -> Option<&BranchIdentity> {
        match self {
            Self::InvalidIdentity => None,
            Self::BeforeCommit { identity, .. } | Self::CommitOutcomeUnknown { identity, .. } => {
                Some(identity)
            }
        }
    }
    /// 无参数；返回本次控制 COMMIT 是否缺少权威确认，不授权自动重发注册或业务。
    pub const fn is_unknown_outcome(&self) -> bool {
        matches!(self, Self::CommitOutcomeUnknown { .. })
    }
}
