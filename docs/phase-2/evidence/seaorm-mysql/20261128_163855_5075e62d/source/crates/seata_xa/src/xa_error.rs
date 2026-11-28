/// XA 标识的固定校验错误，不包含原 XID 或分支文本。
/// 对应 Java: XAXidBuilder/Long.parseLong 的错误边界及 MySQL 参数约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum XaError {
    /// 格式不是固定 Seata XA 格式。
    #[error("unsupported XA format")]
    Format,
    /// 字节编码、规范十进制分支文本或分量不合法。
    #[error("invalid XA components")]
    Components,
    /// MySQL 出站只允许非空 64 字节内全局标识及正原分支号。
    #[error("XA identity is not eligible for MySQL branch operations")]
    MySqlBranch,
}
