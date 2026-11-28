use crate::TmError;
use std::fmt;
/// 原样保留TC提供的非空事务身份，不生成或解析端点，不以浮点处理事务号。
/// 对应 Java: org.apache.seata.common.XID；Rust增加边界与控制字符校验。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Xid(String);
impl Xid {
    /// 参数为原始XID；返回有界身份，空值、边界空白或控制字符返回错误，不做trim改写。
    pub fn parse(value: String) -> Result<Self, TmError> {
        if value.is_empty()
            || value.len() > i16::MAX as usize
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(TmError::InvalidXid {
                reason:
                    "nonempty bounded XID without edge whitespace or control characters required",
            });
        }
        Ok(Self(value))
    }
    /// 返回原始UTF-8身份借用，不裁剪、转义或标准化。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Xid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Xid")
            .field("bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for Xid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl AsRef<str> for Xid {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl TryFrom<String> for Xid {
    type Error = TmError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}
