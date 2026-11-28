use crate::ImageError;

/// MySQL TIME 的有符号微秒时长，独立于一天内的时钟时间。
/// 对应 Java: com.mysql.cj.protocol.InternalTime（物理范围与负号的无损 Rust 表示）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternalTime {
    microseconds: i64,
}

impl InternalTime {
    /// MySQL TIME 的最大绝对微秒数，838:59:59 之后的小数超出真实物理范围。
    pub const MAX_MICROSECONDS: i64 = 3_020_399_000_000;

    /// 参数为有符号微秒数；返回完整物理域内的时长，拒绝溢出而不截断到边界。
    pub fn from_microseconds(microseconds: i64) -> Result<Self, ImageError> {
        if !(-Self::MAX_MICROSECONDS..=Self::MAX_MICROSECONDS).contains(&microseconds) {
            return Err(Self::invalid());
        }
        Ok(Self { microseconds })
    }

    /// 参数为服务器规范时长文本；返回精确到微秒的有符号值，支持两/三位小时和 0–6 位小数。
    pub fn parse_sql(text: &str) -> Result<Self, ImageError> {
        if !(8..=17).contains(&text.len()) {
            return Err(Self::invalid());
        }
        let (negative, clock) = text
            .strip_prefix('-')
            .map_or((false, text), |clock| (true, clock));
        let b = clock.as_bytes();
        let first = b
            .iter()
            .position(|v| *v == b':')
            .ok_or_else(Self::invalid)?;
        if !(2..=3).contains(&first) || b.len() < first + 6 || b[first + 3] != b':' {
            return Err(Self::invalid());
        }
        let digits = |start: usize, end: usize| -> Result<i64, ImageError> {
            b[start..end].iter().try_fold(0, |value, digit| {
                if digit.is_ascii_digit() {
                    Ok(value * 10 + i64::from(digit - b'0'))
                } else {
                    Err(Self::invalid())
                }
            })
        };
        let hours = digits(0, first)?;
        let minutes = digits(first + 1, first + 3)?;
        let seconds = digits(first + 4, first + 6)?;
        let end = first + 6;
        let fraction = if b.len() == end {
            0
        } else {
            let width = b.len() - end - 1;
            if b[end] != b'.' || !(1..=6).contains(&width) {
                return Err(Self::invalid());
            }
            digits(end + 1, b.len())? * 10i64.pow(6 - width as u32)
        };
        if hours > 838 || minutes > 59 || seconds > 59 {
            return Err(Self::invalid());
        }
        let value = (hours * 3600 + minutes * 60 + seconds) * 1_000_000 + fraction;
        Self::from_microseconds(if negative { -value } else { value })
    }

    /// 无参数；返回有符号微秒，负一微秒不会被当作正零或正一微秒。
    pub const fn microseconds(&self) -> i64 {
        self.microseconds
    }

    /// 参数为真实物理 FSP；返回固定小数位数的 SQL 文本，精度不匹配时拒绝而不舍入。
    pub fn to_sql_string(&self, fractional_seconds: u8) -> Result<String, ImageError> {
        if fractional_seconds > 6 {
            return Err(Self::invalid());
        }
        let absolute = self.microseconds.unsigned_abs();
        let seconds = absolute / 1_000_000;
        let fraction = absolute % 1_000_000;
        if fraction % 10u64.pow(6 - u32::from(fractional_seconds)) != 0 {
            return Err(Self::invalid());
        }
        let mut text = format!(
            "{}{:02}:{:02}:{:02}",
            if self.microseconds < 0 { "-" } else { "" },
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        );
        if fractional_seconds != 0 {
            text.push('.');
            text.push_str(&format!("{fraction:06}")[..usize::from(fractional_seconds)]);
        }
        Ok(text)
    }

    fn invalid() -> ImageError {
        ImageError::Invalid {
            reason: "MySQL TIME signed microseconds or fractional precision",
        }
    }
}
