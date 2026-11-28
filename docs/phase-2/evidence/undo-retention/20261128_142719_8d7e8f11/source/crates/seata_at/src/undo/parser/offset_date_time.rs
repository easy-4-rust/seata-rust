use super::{value_format::ValueFormat, UndoCodecError};
use serde_json::Number;

pub(super) const OFFSET_DATE_TIME: &str = "java.time.OffsetDateTime";
const MIN_LOCAL_SECOND: i64 = -31_557_014_135_596_800;
const MAX_LOCAL_SECOND: i64 = 31_556_889_832_780_799;

/// JDBC 有偏移时间，保留向下取整的 UTC 秒、秒内纳秒和原偏移。
/// 对应 Java: java.time.OffsetDateTime。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetDateTime {
    epoch_seconds: i64,
    nanos: u32,
    offset_seconds: i32,
}

impl OffsetDateTime {
    /// 参数为 UTC 秒、秒内纳秒和 UTC 偏移秒，返回经 Java 日期和偏移范围验证的值。
    pub fn from_epoch(
        epoch_seconds: i64,
        nanos: u32,
        offset_seconds: i32,
    ) -> Result<Self, UndoCodecError> {
        let local_seconds = epoch_seconds.checked_add(i64::from(offset_seconds));
        if nanos >= 1_000_000_000
            || !(-64_800..=64_800).contains(&offset_seconds)
            || !local_seconds
                .is_some_and(|local| (MIN_LOCAL_SECOND..=MAX_LOCAL_SECOND).contains(&local))
        {
            return Err(Self::invalid());
        }
        Ok(Self {
            epoch_seconds,
            nanos,
            offset_seconds,
        })
    }

    /// 无参数；返回默认 JavaTimeModule 的数值 JSON 时间戳，保留全部纳秒。
    pub fn encode_jackson_timestamp(&self) -> String {
        if self.epoch_seconds == 0 {
            match self.nanos {
                0 => return "0.0".to_owned(),
                1..=999 => {
                    // BigDecimal 的 scale 为 9；调整后指数小于 -6 时使用科学计数。
                    let digits = self.nanos.to_string();
                    let (first, rest) = digits.split_at(1);
                    let exponent = 10 - digits.len();
                    return if rest.is_empty() {
                        format!("{first}E-{exponent}")
                    } else {
                        format!("{first}.{rest}E-{exponent}")
                    };
                }
                _ => {}
            }
        }
        format!("{}.{:09}", self.epoch_seconds, self.nanos)
    }

    /// 参数为一个 JSON 数值；按固定 Jackson 的秒和纳秒语义解码，返回 UTC 偏移的值。
    /// 默认数值模式不携带源偏移，Jackson 解码按 UTC 上下文恢复。
    pub fn decode_jackson_timestamp(bytes: &[u8]) -> Result<Self, UndoCodecError> {
        if bytes.len() > 96 {
            return Err(UndoCodecError::Budget {
                limit: "OffsetDateTime timestamp bytes",
            });
        }
        let text = std::str::from_utf8(bytes).map_err(|_| UndoCodecError::Syntax)?;
        if !matches!(
            text.trim_start_matches([' ', '\n', '\r', '\t'])
                .as_bytes()
                .first(),
            Some(b'-' | b'0'..=b'9')
        ) {
            return Err(Self::invalid());
        }
        let number: Number = serde_json::from_slice(bytes).map_err(|_| UndoCodecError::Syntax)?;
        Self::decode_number(&number)
    }

    pub(super) fn decode_number(number: &Number) -> Result<Self, UndoCodecError> {
        let text = number.to_string();
        if text.len() > 96 {
            return Err(UndoCodecError::Budget {
                limit: "OffsetDateTime timestamp bytes",
            });
        }
        let negative = text.starts_with('-');
        let unsigned = text.strip_prefix('-').unwrap_or(&text);
        let (mantissa, exponent) = ValueFormat::decimal_parts(unsigned)?;
        let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let digits = format!("{integer}{fraction}");
        let significant = digits.trim_start_matches('0').as_bytes();
        if significant.is_empty() {
            return Self::from_epoch(0, 0, 0);
        }

        // 移动十进制小数点；只读取整数部分和前九位小数，避免浮点及指数驱动分配。
        let point = integer.len() as i64 + exponent - (digits.len() - significant.len()) as i64;
        if point > 17 {
            return Err(Self::invalid());
        }
        let digit_at = |index: i64| {
            usize::try_from(index)
                .ok()
                .and_then(|index| significant.get(index))
                .map_or(0, |digit| digit - b'0')
        };
        let seconds = (0..point).fold(0i64, |seconds, index| {
            seconds * 10 + i64::from(digit_at(index))
        });
        let nanos =
            (point..point + 9).fold(0u32, |nanos, index| nanos * 10 + u32::from(digit_at(index)));
        // 固定 Jackson 对负整数秒保留小数位的绝对值；仅负零秒需要借一秒。
        if negative && seconds == 0 && nanos != 0 {
            return Self::from_epoch(-1, 1_000_000_000 - nanos, 0);
        }
        Self::from_epoch(if negative { -seconds } else { seconds }, nanos, 0)
    }

    fn invalid() -> UndoCodecError {
        UndoCodecError::Shape {
            reason: "OffsetDateTime timestamp range or type",
        }
    }
}
