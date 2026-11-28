/// 行镜像的结构与预算错误；固定原因不包含数据库字段值。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.TableRecords#buildRecords的SQLException边界；Rust扩展。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImageError {
    /// 字段或行不符合精确值域和固定schema。
    #[error("invalid row image: {reason}")]
    Invalid {
        /// 固定原因，不包含用户值。
        reason: &'static str,
    },
    /// 负载或对象数量超过有限预算。
    #[error("row image budget exceeded: {limit}")]
    Budget {
        /// 固定预算名称。
        limit: &'static str,
    },
}
