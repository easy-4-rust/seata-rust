use seata_sqlparser::{JavaObjectError, JavaString, MetadataError};
use std::sync::Arc;

/// 原 Fastjson 比较中的目标日期时间类。
/// 对应 Java: java.sql.Date、Time、Timestamp 与 java.time.LocalDateTime。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JavaCompareTemporal {
    /// java.sql.Date。
    Date,
    /// java.sql.Time。
    Time,
    /// java.sql.Timestamp。
    Timestamp,
    /// java.time.LocalDateTime。
    LocalDateTime,
}

impl JavaCompareTemporal {
    /// 解析原 Fastjson 字段转换需要的 UTF-16 文本，返回同类型比较用表示。
    /// JDBC 值采用 UTC，LocalDateTime 保留本地日历字段与纳秒。
    /// 对应 Java: DataCompareUtils#convertType 使用的 JDBC valueOf 与 LocalDateTime#parse。
    pub fn parse_fastjson_utc(self, text: &JavaString) -> Result<JavaString, MetadataError> {
        if self == Self::LocalDateTime {
            parse_local_datetime(text.as_utf16()).ok_or_else(|| {
                MetadataError::ObjectException(Arc::new(
                    JavaObjectError::new("java.time.format.DateTimeParseException", None)
                        .with_superclasses([
                            "java.time.DateTimeException",
                            "java.lang.RuntimeException",
                            "java.lang.Exception",
                            "java.lang.Throwable",
                        ]),
                ))
            })
        } else {
            self.parse_jdbc_utc(text)
        }
    }

    /// 参数为 JDBC 日期时间类型及原 UTF-16 文本，返回 UTC 纪元秒与九位纳秒。
    /// 固定 JDK 17 的 JDBC 文本语法与日历进位。应以同一入口取得两个值的身份表示。
    /// 对应 Java: java.sql.Date#valueOf、Time#valueOf 与 Timestamp#valueOf。
    pub fn parse_jdbc_utc(self, text: &JavaString) -> Result<JavaString, MetadataError> {
        let input = text.as_utf16();
        let (date, seconds, nanos) = match self {
            Self::Date => (parse_date(input, date_error)?, 0, 0),
            Self::Time => {
                let (seconds, nanos) = parse_clock(input, false, date_error)?;
                ((1970, 1, 1), seconds, nanos)
            }
            Self::Timestamp => {
                let trimmed = text.java_trim();
                let (date, clock) = split(trimmed.as_utf16(), b' ').ok_or_else(timestamp_error)?;
                let date = parse_date(date, timestamp_error)?;
                let (seconds, nanos) = parse_clock(clock, true, timestamp_error)?;
                (date, seconds, nanos)
            }
            Self::LocalDateTime => {
                return Err(MetadataError::InvalidCompatibilityData(
                    "LocalDateTime requires a separate host",
                ));
            }
        };
        let seconds = jdbc_epoch_seconds(date, seconds);
        Ok(format!("{seconds}:{nanos:09}").into())
    }
}

fn parse_local_datetime(text: &[u16]) -> Option<JavaString> {
    let separator = text.iter().position(|unit| matches!(unit, 0x54 | 0x74))?;
    let date = &text[..separator];
    let clock = &text[separator + 1..];
    let (sign, date) = match date {
        [0x2b, rest @ ..] => (Some(1_i32), rest),
        [0x2d, rest @ ..] => (Some(-1_i32), rest),
        _ => (None, date),
    };
    let (year, date) = split(date, b'-')?;
    let valid_width = match sign {
        Some(1) => (5..=10).contains(&year.len()),
        Some(-1) => (4..=10).contains(&year.len()),
        _ => year.len() == 4,
    };
    if !valid_width {
        return None;
    }
    let year = i32::try_from(ascii_digits(year)?).ok()? * sign.unwrap_or(1);
    if !(-999_999_999..=999_999_999).contains(&year) || (sign == Some(-1) && year == 0) {
        return None;
    }
    let [mo1, mo2, 0x2d, d1, d2] = date else {
        return None;
    };
    let month = ascii_digits(&[*mo1, *mo2])?;
    let day = ascii_digits(&[*d1, *d2])?;
    let days_in_month = match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return None,
    };
    if !(1..=days_in_month).contains(&day) {
        return None;
    }

    let [h1, h2, 0x3a, mi1, mi2, tail @ ..] = clock else {
        return None;
    };
    let hour = ascii_digits(&[*h1, *h2])?;
    let minute = ascii_digits(&[*mi1, *mi2])?;
    let (second, nanos) = match tail {
        [] => (0, 0),
        [0x3a, s1, s2, tail @ ..] => {
            let second = ascii_digits(&[*s1, *s2])?;
            let nanos = match tail {
                [] => 0,
                [0x2e, fraction @ ..] if fraction.len() <= 9 => {
                    ascii_digits(fraction)? * 10_u32.pow(9 - fraction.len() as u32)
                }
                _ => return None,
            };
            (second, nanos)
        }
        _ => return None,
    };
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(format!("{year}:{month:02}:{day:02}:{hour:02}:{minute:02}:{second:02}:{nanos:09}").into())
}

fn ascii_digits(digits: &[u16]) -> Option<u32> {
    digits.iter().try_fold(0_u32, |value, &unit| {
        let digit = unit.checked_sub(0x30)?;
        if digit > 9 {
            return None;
        }
        value.checked_mul(10)?.checked_add(u32::from(digit))
    })
}

fn split(input: &[u16], delimiter: u8) -> Option<(&[u16], &[u16])> {
    let point = input
        .iter()
        .position(|&unit| unit == u16::from(delimiter))?;
    Some((&input[..point], &input[point + 1..]))
}

fn parse_date(
    input: &[u16],
    format_error: fn() -> MetadataError,
) -> Result<(i32, i32, i32), MetadataError> {
    let (year, remaining) = split(input, b'-').ok_or_else(format_error)?;
    let (month, day) = split(remaining, b'-').ok_or_else(format_error)?;
    if year.len() != 4 || !(1..=2).contains(&month.len()) || !(1..=2).contains(&day.len()) {
        return Err(format_error());
    }
    let (year, month, day) = (parse_int(year)?, parse_int(month)?, parse_int(day)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format_error());
    }
    Ok((year, month, day))
}

fn parse_clock(
    input: &[u16],
    timestamp: bool,
    format_error: fn() -> MetadataError,
) -> Result<(i64, i32), MetadataError> {
    let (hours, remaining) = split(input, b':').ok_or_else(format_error)?;
    let (minutes, seconds) = split(remaining, b':').ok_or_else(format_error)?;
    if seconds.is_empty() || (!timestamp && hours.is_empty()) {
        return Err(format_error());
    }
    let hours = i64::from(parse_int(hours)?);
    let minutes = i64::from(parse_int(minutes)?);
    let (seconds, nanos) = match timestamp.then(|| split(seconds, b'.')).flatten() {
        Some((whole, fraction)) => {
            if fraction.is_empty() {
                return Err(format_error());
            }
            let whole = parse_int(whole)?;
            if fraction.len() > 9 || decimal_digit(fraction[0]).is_none() {
                return Err(format_error());
            }
            (
                whole,
                parse_int(fraction)? * 10_i32.pow(9 - fraction.len() as u32),
            )
        }
        None => (parse_int(seconds)?, 0),
    };
    Ok(((hours * 60 + minutes) * 60 + i64::from(seconds), nanos))
}

fn parse_int(input: &[u16]) -> Result<i32, MetadataError> {
    let (sign, digits) = match input {
        [0x2d, digits @ ..] => (-1, digits),
        [0x2b, digits @ ..] => (1, digits),
        digits => (1, digits),
    };
    if digits.is_empty() {
        return Err(number_error());
    }
    let mut value = 0_i32;
    for &unit in digits {
        let digit = decimal_digit(unit).ok_or_else(number_error)?;
        value = value
            .checked_mul(10)
            .and_then(|previous| previous.checked_add(sign * digit))
            .ok_or_else(number_error)?;
    }
    Ok(value)
}

fn decimal_digit(unit: u16) -> Option<i32> {
    // 固定 JDK 17 Character.digit(char, 10) 的 BMP 十进制零位。
    const ZEROS: &[u16] = &[
        0x0030, 0x0660, 0x06f0, 0x07c0, 0x0966, 0x09e6, 0x0a66, 0x0ae6, 0x0b66, 0x0be6, 0x0c66,
        0x0ce6, 0x0d66, 0x0de6, 0x0e50, 0x0ed0, 0x0f20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946,
        0x19d0, 0x1a80, 0x1a90, 0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0,
        0xa9f0, 0xaa50, 0xabf0, 0xff10,
    ];
    let end = ZEROS.partition_point(|&zero| zero <= unit);
    let digit = unit - ZEROS[end.checked_sub(1)?];
    (digit < 10).then_some(i32::from(digit))
}

fn jdbc_epoch_seconds((year, month, day): (i32, i32, i32), seconds: i64) -> i64 {
    const CUTOVER: i64 = -12_219_292_800;
    // JDK 默认儒略历／公历切换为 1582-10-15；非切换年若进位跨界，按另一历法重算。
    let shift = (14 - i64::from(month)) / 12;
    let years = i64::from(year) + 4800 - shift;
    let month = i64::from(month) + 12 * shift - 3;
    let common = i64::from(day) + (153 * month + 2) / 5 + 365 * years + years / 4;
    let gregorian = (common - years / 100 + years / 400 - 32045 - 2440588) * 86400 + seconds;
    let julian = (common - 32083 - 2440588) * 86400 + seconds;
    let use_gregorian = if year < 1582 {
        julian >= CUTOVER
    } else {
        gregorian >= CUTOVER
    };
    if use_gregorian {
        gregorian
    } else {
        julian
    }
}

fn jdbc_error(class: &str, message: Option<&str>) -> MetadataError {
    let error = JavaObjectError::new(class, message.map(JavaString::from));
    let error = if class == "java.lang.NumberFormatException" {
        error.with_superclasses([
            "java.lang.IllegalArgumentException",
            "java.lang.RuntimeException",
            "java.lang.Exception",
            "java.lang.Throwable",
        ])
    } else {
        error.with_superclasses([
            "java.lang.RuntimeException",
            "java.lang.Exception",
            "java.lang.Throwable",
        ])
    };
    MetadataError::ObjectException(Arc::new(error))
}

fn date_error() -> MetadataError {
    jdbc_error("java.lang.IllegalArgumentException", None)
}

fn timestamp_error() -> MetadataError {
    jdbc_error(
        "java.lang.IllegalArgumentException",
        Some("Timestamp format must be yyyy-mm-dd hh:mm:ss[.fffffffff]"),
    )
}

fn number_error() -> MetadataError {
    jdbc_error("java.lang.NumberFormatException", None)
}
