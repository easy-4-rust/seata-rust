/// 数据源元数据的本地不变量错误，不携带SQL或连接凭据。
/// 对应 Java: org.apache.seata.rm.datasource.exception.TableMetaException；Rust细分本地校验与预算。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MetadataError {
    /// 元数据不满足已声明结构。
    #[error("invalid metadata: {reason}")]
    Invalid {
        /// 固定原因，不插入用户标识符或值。
        reason: &'static str,
    },
    /// 元数据超出实现资源预算。
    #[error("metadata budget exceeded: {limit}")]
    Budget {
        /// 固定预算名称。
        limit: &'static str,
    },
}
