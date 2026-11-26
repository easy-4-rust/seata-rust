/// 原主键锁键生成与累计预算错误；不携带原表、主键、SQL或字段值。
/// 对应 Java: BaseTransactionalExecutor#validPk的IllegalArgumentException；Rust有界错误扩展。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LockKeyError {
    /// 参数或主键不能构成所选原格式。
    #[error("invalid lock key: {reason}")]
    Invalid {
        /// 固定原因，不包含原输入。
        reason: &'static str,
    },
    /// 原生字段/JDBC配置尚不能无损映射。
    #[error("unsupported lock key: {feature}")]
    Unsupported {
        /// 固定功能名称。
        feature: &'static str,
    },
    /// 单键、累计键或日志数超出硬预算。
    #[error("lock key budget exceeded: {limit}")]
    Budget {
        /// 固定预算名称。
        limit: &'static str,
    },
}
