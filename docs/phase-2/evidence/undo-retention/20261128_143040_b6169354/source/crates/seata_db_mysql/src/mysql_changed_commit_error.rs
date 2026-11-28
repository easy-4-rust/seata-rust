use crate::{DbError, MySqlRecoveryError};
use seata_rm::RegisteredBranch;

/// 实验性有变化一阶段本地提交错误；明确区分发送前拒绝与提交可能已生效。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 的本地结果分类；Rust 扩展。
#[derive(Debug, thiserror::Error)]
pub enum MySqlChangedCommitError {
    /// 原回执、Undo、见证或独立观察者不匹配；未发送本次 COMMIT。
    #[error("changed MySQL COMMIT is not eligible: {reason}")]
    NotEligible {
        /// 原真实成功登记回执；无回执时不会合成身份。
        receipt: Option<Box<RegisteredBranch>>,
        /// 静态拒绝原因，不含业务 SQL 或凭据。
        reason: &'static str,
    },
    /// 独立尝试记录未确认；原业务 COMMIT 没有发送，回滚结果单独保留。
    #[error("changed MySQL COMMIT was not sent because durable arming failed: {source}")]
    Arming {
        /// 原真实回执；不能重新登记或伪造分支编号。
        receipt: Box<RegisteredBranch>,
        /// 尝试记录的明确失败或控制 COMMIT 结果未知。
        #[source]
        source: MySqlRecoveryError,
        /// 原业务事务的显式回滚结果；失败时不能报告明确一阶段失败。
        rollback: Result<(), Box<DbError>>,
    },
    /// COMMIT 已进入驱动或已确认但独立见证未确认；禁止重放业务。
    #[error("changed MySQL COMMIT outcome needs recovery observation: {source}")]
    Unknown {
        /// 保留原真实成功登记回执，供独立恢复与 TC 核对。
        receipt: Box<RegisteredBranch>,
        /// 原驱动或独立观察错误。
        #[source]
        source: DbError,
    },
}
impl MySqlChangedCommitError {
    /// 无参数；返回已真实取得的原回执，未取得时为 None。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        match self {
            Self::NotEligible { receipt, .. } => receipt.as_deref(),
            Self::Arming { receipt, .. } | Self::Unknown { receipt, .. } => Some(receipt),
        }
    }

    /// 无参数；返回本次 COMMIT 是否可能已发送或持久生效。
    pub fn is_unknown_outcome(&self) -> bool {
        match self {
            Self::Unknown { .. } => true,
            Self::Arming {
                source, rollback, ..
            } => source.is_unknown_outcome() || rollback.is_err(),
            Self::NotEligible { .. } => false,
        }
    }
}
