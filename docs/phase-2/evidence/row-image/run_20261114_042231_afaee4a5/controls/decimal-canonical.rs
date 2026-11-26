use crate::ImageError;
use std::fmt;

/// 精确普通十进制值，保留小数末尾零，禁止浮点中转。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field#value中的BigDecimal；Rust规范字符串扩展。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DecimalValue {
    text: String,
    integer_digits: u32,
    scale: u32,
}
impl DecimalValue {
    /// 参数为规范十进制文本；返回最多65位容量/30位scale的精确值，错误不回显输入。
    pub fn new(text: &str) -> Result<Self, ImageError> {
        // 先检查原字节预算，再扫描和复制；符号、小数点及整数零最多额外占3字节。
        if text.len() > 68 {
            return Err(ImageError::Budget {
                limit: "DECIMAL text",
            });
        }
        let unsigned = text.strip_prefix('-').or_else(||text.strip_prefix('+')).unwrap_or(text);
        let (integer, fraction) = match unsigned.split_once('.') {
            Some((integer, fraction)) => (integer, Some(fraction)),
            None => (unsigned, None),
        };
        if integer.is_empty()
            || !integer.bytes().all(|v| v.is_ascii_digit())
            || (integer.len() > 1 && integer.starts_with('0'))
            || fraction.is_some_and(|v| v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(ImageError::Invalid {
                reason: "canonical DECIMAL required",
            });
        }
        let integer_digits = if integer == "0" {
            0
        } else {
            integer.len() as u32
        };
        let scale = fraction.map_or(0, |v| v.len() as u32);
        if scale > 30 || integer_digits + scale > 65 {
            return Err(ImageError::Budget {
                limit: "DECIMAL precision/scale",
            });
        }
        if text.starts_with('-') && unsigned.bytes().all(|v| v == b'0' || v == b'.') {
            return Err(ImageError::Invalid {
                reason: "negative zero is not canonical",
            });
        }
        Ok(Self {
            text: text.to_owned(),
            integer_digits,
            scale,
        })
    }
    /// 无参数；返回完整精确十进制文本，含原有小数末尾零。
    pub fn as_str(&self) -> &str {
        &self.text
    }
    /// 无参数；返回非前导整数位数，整数零为0位，供DECIMAL(p,p)容量验证。
    pub const fn integer_digits(&self) -> u32 {
        self.integer_digits
    }
    /// 无参数；返回小数位数。
    pub const fn scale(&self) -> u32 {
        self.scale
    }
}
impl fmt::Debug for DecimalValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecimalValue")
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}
