use crate::{DecimalValue, InternalTime, LocalDate, LocalDateTime, Timestamp};
use std::fmt;

/// 精确且可脱敏的数据库值域，NULL与空值独立。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field#value的Object；Rust穷尽值域扩展。
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum FieldValue {
    /// MySQL DATE 日历日期。
    LocalDate(LocalDate),
    /// MySQL TIME 有符号微秒时长。
    InternalTime(InternalTime),
    /// MySQL TIMESTAMP 的 UTC 时间点。
    Timestamp(Timestamp),
    /// MySQL DATETIME 日历值，不经浮点数和主机时区。
    LocalDateTime(LocalDateTime),
    /// 数据库NULL。
    Null,
    /// 精确有符号64位整数。
    Signed(i64),
    /// 精确无符号64位整数。
    Unsigned(u64),
    /// UTF-8文本，不进行Unicode规范化。
    Text(String),
    /// 任意二进制字节，不进行字符解码。
    Binary(Vec<u8>),
    /// 具有精确scale的十进制值。
    Decimal(DecimalValue),
}
impl FieldValue {
    /// 无参数；返回值负载字节，不包含容器、列名或元数据开销。
    pub fn value_bytes(&self) -> usize {
        match self {
            Self::LocalDate(_) => 10,
            Self::InternalTime(_) => 17,
            Self::Timestamp(_) | Self::LocalDateTime(_) => 26,
            Self::Null => 0,
            Self::Signed(_) | Self::Unsigned(_) => 8,
            Self::Text(v) => v.len(),
            Self::Binary(v) => v.len(),
            Self::Decimal(v) => v.as_str().len(),
        }
    }
}
impl fmt::Debug for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::LocalDate(_) => "LocalDate",
            Self::InternalTime(_) => "InternalTime",
            Self::Timestamp(_) => "Timestamp",
            Self::LocalDateTime(_) => "LocalDateTime",
            Self::Null => "Null",
            Self::Signed(_) => "Signed",
            Self::Unsigned(_) => "Unsigned",
            Self::Text(_) => "Text",
            Self::Binary(_) => "Binary",
            Self::Decimal(_) => "Decimal",
        };
        f.debug_struct("FieldValue")
            .field("kind", &kind)
            .field("bytes", &self.value_bytes())
            .finish_non_exhaustive()
    }
}
