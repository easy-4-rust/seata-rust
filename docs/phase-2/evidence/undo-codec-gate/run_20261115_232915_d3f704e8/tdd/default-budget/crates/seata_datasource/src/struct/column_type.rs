/// 当前元数据可表示的精确列值域，不以此声称业务行值已读取或事务已提交。
/// 对应 Java: java.sql.Types与ColumnMeta#dataType的Rust显式映射；尚不覆盖全部JDBC类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    /// 有符号整数，读取时使用i64保持精度。
    SignedInteger,
    /// 无符号整数，读取时使用u64保持精度。
    UnsignedInteger,
    /// 文本，不把二进制列解码为字符串。
    Text,
    /// 二进制，不进行UTF8规范化。
    Binary,
    /// 精确DECIMAL，精度及scale单独保留。
    Decimal,
}
