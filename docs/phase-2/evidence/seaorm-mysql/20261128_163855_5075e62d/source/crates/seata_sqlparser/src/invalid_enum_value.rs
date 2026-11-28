use std::{error::Error, fmt};

/// Rust 扩展：保留 Java 枚举 valueOf 的 IllegalArgumentException 类别及原消息。
/// 对应 Java: SQLType#valueOf、IndexType#valueOf；Rust 辅助错误，不计入 Java 对象分母。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidEnumValue {
    enum_name: &'static str,
    value: i32,
}

impl InvalidEnumValue {
    pub(crate) const fn new(enum_name: &'static str, value: i32) -> Self {
        Self { enum_name, value }
    }

    /// 返回被查询的 Java 枚举名；无参数。
    pub const fn enum_name(&self) -> &'static str {
        self.enum_name
    }

    /// 返回拒绝的原始有符号整数；无参数。
    pub const fn value(&self) -> i32 {
        self.value
    }
}

impl fmt::Display for InvalidEnumValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid {}:{}", self.enum_name, self.value)
    }
}

impl Error for InvalidEnumValue {}
