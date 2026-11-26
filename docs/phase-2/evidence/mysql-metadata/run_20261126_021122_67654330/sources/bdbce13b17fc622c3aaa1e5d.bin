use seata_rm::RmCallError;

/// 已注册准备错误的失败报告调用结果；原准备错误及回执仍由调用方持有。
/// 对应 Java: AbstractResourceManager#branchReport；Rust 区分不适用、重复与原 RM 调用结果。
#[derive(Debug, thiserror::Error)]
pub enum MySqlPrepareFailureReportError {
    /// 本地显式回滚没有成功确认，或不存在真实登记回执；没有发送报告。
    #[error("preparation failure report requires confirmed rollback and registered branch")]
    NotEligible,
    /// 同一准备错误此前已进入报告调用；响应丢失或取消后不允许重发。
    #[error("preparation failure report was already attempted")]
    AlreadyAttempted,
    /// 原 RM 报告明确拒绝、发送前失败或结果未知；完整分类由原错误保留。
    #[error("preparation failure report failed: {0}")]
    Call(#[from] RmCallError),
}

impl MySqlPrepareFailureReportError {
    /// 无参数；返回是否必须保守按先前调用可能已发送处理。
    /// 重复调用无法获知前一次的响应，即使前次曾成功也保守返回 true，不授权重发。
    pub fn is_unknown_outcome(&self) -> bool {
        match self {
            Self::NotEligible => false,
            Self::AlreadyAttempted => true,
            Self::Call(source) => source.is_unknown_outcome(),
        }
    }
}
