/// AT当前可识别类型的SQL分类，明确保留Java编号；其他语句不降级为写操作。
/// 对应 Java: org.apache.seata.sqlparser.SQLType的INSERT/UPDATE/DELETE。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlType {
    /// INSERT VALUES。
    Insert,
    /// UPDATE单表。
    Update,
    /// DELETE单表。
    Delete,
}
impl SqlType {
    /// 无参数；返回Java SQLType原始编号，不使用Rust枚举序号转换。
    pub const fn java_code(self) -> i32 {
        match self {
            Self::Insert => 1,
            Self::Update => 2,
            Self::Delete => 3,
        }
    }
}
