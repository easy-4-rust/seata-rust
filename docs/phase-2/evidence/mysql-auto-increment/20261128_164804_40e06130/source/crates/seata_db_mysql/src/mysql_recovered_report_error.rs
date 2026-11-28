use crate::{DbError, MySqlCommitRecoveryError};
use seata_rm::RmCallError;

/// 新进程按持久原身份报告一阶段成功时的分层结果；不签发原 TC 回执。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 与 AbstractResourceManager#branchReport。
#[derive(Debug, thiserror::Error)]
pub enum MySqlRecoveredReportError {
    /// 原本地提交尚未确认，禁止报告。
    #[error("original MySQL local commit recovery remains unknown: {0}")]
    Recovery(#[from] MySqlCommitRecoveryError),
    /// 持久围栏已证明原业务未提交，禁止成功报告。
    #[error("original MySQL local commit was not applied and was fenced")]
    NotAppliedFenced,
    /// 已确认提交，但原持久身份或发送门闩未能确认；禁止报告。
    #[error("original MySQL success report claim remains unknown: {source}")]
    Claim {
        /// 脱敏的数据库诊断。
        #[source]
        source: DbError,
    },
    /// 报告可能已由任一进程发送，禁止再次报告。
    #[error("original MySQL success report may already have been sent")]
    AlreadyStarted,
    /// 门闩已确认，原 TC 报告调用返回原始错误；绝不重发。
    #[error("original TC success report did not confirm success: {source}")]
    Report {
        /// 原协议报告错误。
        #[source]
        source: Box<RmCallError>,
    },
}

impl MySqlRecoveredReportError {
    /// 无参数；返回是否因原提交、门闩或报告响应未确认而不能发布终态。
    pub fn is_unknown_outcome(&self) -> bool {
        match self {
            Self::NotAppliedFenced => false,
            Self::Report { source } => source.is_unknown_outcome(),
            Self::Recovery(_) | Self::Claim { .. } | Self::AlreadyStarted => true,
        }
    }
}
