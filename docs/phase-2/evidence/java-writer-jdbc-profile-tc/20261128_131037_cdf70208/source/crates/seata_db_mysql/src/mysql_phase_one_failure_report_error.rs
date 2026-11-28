use crate::DbError;
use seata_rm::{RegisteredBranch, RmCallError};

/// 明确本地回滚与原 TC 失败报告的分层错误，保留真实登记回执。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 与 AbstractResourceManager#branchReport。
#[derive(Debug, thiserror::Error)]
pub enum MySqlPhaseOneFailureReportError {
    /// 原本地回滚没有得到成功确认，不允许发送失败报告。
    #[error("MySQL local rollback was not confirmed: {source}")]
    Local {
        /// 原分支成功登记回执；无变化作用域可以为空。
        receipt: Option<Box<RegisteredBranch>>,
        /// 原本地回滚错误。
        #[source]
        source: DbError,
    },
    /// 本地回滚已确认，但原 TC 报告没有成功确认。
    #[error("MySQL local rollback confirmed, TC failure report failed: {source}")]
    Report {
        /// 已回滚分支的原登记回执。
        receipt: Box<RegisteredBranch>,
        /// 单次报告调用的原错误。
        #[source]
        source: Box<RmCallError>,
    },
}

impl MySqlPhaseOneFailureReportError {
    /// 无参数；返回原登记回执，未发生分支登记时返回 None。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        match self {
            Self::Local { receipt, .. } => receipt.as_deref(),
            Self::Report { receipt, .. } => Some(receipt),
        }
    }

    /// 无参数；返回原本地事务回滚是否已收到成功确认。
    pub const fn local_rollback_confirmed(&self) -> bool {
        matches!(self, Self::Report { .. })
    }

    /// 无参数；返回回滚或报告可能已生效但没有权威确认的保守分类。
    pub fn is_unknown_outcome(&self) -> bool {
        match self {
            Self::Local { .. } => true,
            Self::Report { source, .. } => source.is_unknown_outcome(),
        }
    }
}
