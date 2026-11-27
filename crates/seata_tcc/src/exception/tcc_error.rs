/// TCC 上下文与 Fence 输入拒绝；错误不包含业务值、原 XID 或资源文本。
/// 对应 Java: org.apache.seata.integration.tx.api.fence.exception.CommonFenceException；Rust 输入错误扩展。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TccError {
    /// 业务明确失败；持久适配器必须回滚本地 Fence 与业务，不能发布完成。
    #[error("TCC business action failed")]
    Business,
    /// 事务不可继续或执行结果未确认，不向业务公开 SQL/驱动输入。
    #[error("TCC transaction unavailable")]
    TransactionUnavailable,
    /// 原应用数据不是可接受的 JSON。
    #[error("invalid TCC context JSON")]
    InvalidContext,
    /// actionContext 必须是对象或 null。
    #[error("TCC action context must be an object or null")]
    ContextNotObject,
    /// 原字节或结构超过明确预算。
    #[error("TCC context budget exhausted")]
    ContextBudget,
    /// 不认识的持久 Fence 码，不能猜测为任何已知状态。
    #[error("unknown TCC fence status")]
    UnknownFenceStatus,
}
