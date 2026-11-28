use crate::DbError;
use seata_protocol::GlobalStatus;
use seata_tm::TmError;

/// TM 持久代际申请失败原因。对应 Java 无直接类型。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MySqlLauncherClaimError {
    /// 来源或停止证明冲突，或 MySQL 未确认代际更新。
    #[error(transparent)]
    Store(#[from] DbError),
    /// 原 TC 查询失败，保留原协议错误。
    #[error("无法查询原 TC：{0}")]
    Query(#[source] Box<TmError>),
    /// TC 返回的状态不允许继续申请代际，保留原状态。
    #[error("TC 状态 {0:?} 不允许代际更新")]
    Observed(GlobalStatus),
}
