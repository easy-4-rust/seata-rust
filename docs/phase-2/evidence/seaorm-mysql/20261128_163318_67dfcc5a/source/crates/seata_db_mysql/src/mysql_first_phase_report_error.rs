use crate::{DbError, MySqlChangedCommitError};
use seata_rm::{RegisteredBranch, RmCallError};

/// 实验一阶段提交及报告的分层错误；报告故障不改写已确认的本地 COMMIT。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 与 AbstractResourceManager#branchReport。
#[derive(Debug, thiserror::Error)]
pub enum MySqlFirstPhaseReportError {
    /// 本地提交未确认，绝不发送成功报告。
    #[error("changed MySQL local COMMIT was not confirmed: {0}")]
    Local(#[from] MySqlChangedCommitError),
    /// 本地提交已确认，但持久报告门闩未确认或已由其他进程抢占。
    #[error("changed MySQL local COMMIT confirmed, TC report claim remains unknown: {source}")]
    Claim {
        /// 原成功登记回执。
        receipt: Box<RegisteredBranch>,
        /// 数据库失败或重复报告诊断。
        #[source]
        source: DbError,
    },
    /// 本地 COMMIT 与独立见证已确认；原报告响应未获成功确认。
    #[error("changed MySQL local COMMIT confirmed, TC report failed: {source}")]
    Report {
        /// 已确认本地提交的原登记回执。
        receipt: Box<RegisteredBranch>,
        /// 单次原报告调用的完整错误与请求/响应身份。
        #[source]
        source: Box<RmCallError>,
    },
}
impl MySqlFirstPhaseReportError {
    /// 无参数；返回真实登记回执，未取得时为 None。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        match self {
            Self::Local(error) => error.registered_branch(),
            Self::Claim { receipt, .. } | Self::Report { receipt, .. } => Some(receipt),
        }
    }

    /// 无参数；返回本地 COMMIT 和独立见证是否均已确认。
    pub const fn local_commit_confirmed(&self) -> bool {
        matches!(self, Self::Claim { .. } | Self::Report { .. })
    }

    /// 无参数；返回本地提交或报告是否可能已生效但缺少权威响应。
    pub fn is_unknown_outcome(&self) -> bool {
        match self {
            Self::Local(error) => error.is_unknown_outcome(),
            Self::Claim { .. } => true,
            Self::Report { source, .. } => source.is_unknown_outcome(),
        }
    }
}
