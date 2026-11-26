use seata_protocol::transaction::AbstractTransactionResponse;
use seata_protocol::GlobalStatus;
/// 一次有效TC状态响应，完整保留结果前缀与状态，不把Success或Finished等同已提交。
/// 对应 Java: GlobalCommitResponse/GlobalRollbackResponse/GlobalStatusResponse的共同观察结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionOutcome {
    pub(crate) response: AbstractTransactionResponse,
    pub(crate) status: GlobalStatus,
}
impl TransactionOutcome {
    /// 返回包括可选消息、异常码及ResultCode的原始前缀。
    pub const fn response(&self) -> &AbstractTransactionResponse {
        &self.response
    }
    /// 返回TC实际状态；异步进行、重试时限及Finished均不被改写。
    pub const fn status(&self) -> GlobalStatus {
        self.status
    }
}
