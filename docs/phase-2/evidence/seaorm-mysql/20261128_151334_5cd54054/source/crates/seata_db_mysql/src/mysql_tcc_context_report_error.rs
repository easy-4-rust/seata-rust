use crate::DbError;
use seata_rm::RmCallError;
/// 动态上下文报告失败；保留 ADR-0008 的拒绝、未发送和未知响应，不展示原业务值。
/// 对应 Java: BusinessActionContextUtil#reportContext 的异常边界；Rust 类型化错误扩展。
#[derive(Debug, thiserror::Error)]
pub enum MySqlTccContextReportError {
    /// 原版本/结构/本地持久化未确认，没有据此否定已有报告效果。
    #[error("TCC context version persistence failed")]
    Local(#[from] DbError),
    /// 正式单次 BranchReport 的完整类型化结果。
    #[error(transparent)]
    Rpc(#[from] RmCallError),
}
