use crate::ImageError;

/// MySQL DATETIME 的精确 Gregorian 日历值，支持 1000–9999 年及微秒，不依赖宿主时区。
/// 对应 Java: java.time.LocalDateTime（MySQL 已认证日历范围）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalDateTime {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
}

impl LocalDateTime {
    /// 参数为日历字段和秒内纳秒；返回合法 MySQL 值，拒绝零日期及亚微秒精度。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        nanosecond: u32,
    ) -> Result<Self, ImageError> {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => 0,
        };
        if !(1000..=9999).contains(&year)
            || day == 0
            || day > days
            || hour > 23
            || minute > 59
            || second > 59
            || nanosecond >= 1_000_000_000
            || nanosecond % 1000 != 0
        {
            return Err(Self::invalid());
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            nanosecond,
        })
    }
    /// 参数为服务器完整 DATETIME 文本；返回严格消费的日历值，允许 0–6 位小数秒。
    pub fn parse_sql(text: &str) -> Result<Self, ImageError> {
        let b = text.as_bytes();
        if !(19..=26).contains(&b.len())
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b' '
            || b[13] != b':'
            || b[16] != b':'
            || (b.len() > 19 && (b.len() == 20 || b[19] != b'.'))
        {
            return Err(Self::invalid());
        }
        let digits = |start: usize, end: usize| -> Result<u32, ImageError> {
            b[start..end].iter().try_fold(0u32, |v, c| {
                if c.is_ascii_digit() {
                    Ok(v * 10 + u32::from(c - b'0'))
                } else {
                    Err(Self::invalid())
                }
            })
        };
        let nanos = if b.len() > 19 {
            digits(20, b.len())? * 10u32.pow(9 - (b.len() - 20) as u32)
        } else {
            0
        };
        Self::new(
            digits(0, 4)? as i32,
            digits(5, 7)? as u8,
            digits(8, 10)? as u8,
            digits(11, 13)? as u8,
            digits(14, 16)? as u8,
            digits(17, 19)? as u8,
            nanos,
        )
    }
    /// 参数为将日历按 UTC 投影的纪元秒及秒内纳秒；返回完整范围检查后的日历值。
    pub fn from_epoch_seconds(seconds: i64, nanosecond: u32) -> Result<Self, ImageError> {
        let z = seconds.div_euclid(86400) + 719468;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let year = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = mp + if mp < 10 { 3 } else { -9 };
        let year = year + i64::from(month <= 2);
        let clock = seconds.rem_euclid(86400);
        Self::new(
            i32::try_from(year).map_err(|_| Self::invalid())?,
            month as u8,
            day as u8,
            (clock / 3600) as u8,
            ((clock % 3600) / 60) as u8,
            (clock % 60) as u8,
            nanosecond,
        )
    }
    /// 无参数；返回将本地日历视为 UTC 后向下取整的纪元秒，不转换宿主时区。
    pub fn epoch_seconds(&self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let mp = m + if m > 2 { -3 } else { 9 };
        let doy = (153 * mp + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        (era * 146097 + doe - 719468) * 86400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }
    /// 无参数；返回原秒内纳秒；已保证与微秒对齐。
    pub const fn nanosecond(&self) -> u32 {
        self.nanosecond
    }
    /// 无参数；返回完整 SQL 日历文本，非零小数使用六位，适用于绑定和严格解析。
    pub fn to_sql_string(&self) -> String {
        let mut s = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        );
        if self.nanosecond != 0 {
            s.push_str(&format!(".{:06}", self.nanosecond / 1000));
        }
        s
    }
    /// 无参数；返回 Java LocalDateTime#toString 的主键字符串，保持秒和小数的省略规则。
    pub fn to_java_string(&self) -> String {
        let mut s = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute
        );
        if true {
            s.push_str(&format!(":{:02}", self.second));
        }
        if self.nanosecond != 0 {
            s.push_str(&if self.nanosecond % 1_000_000 == 0 {
                format!(".{:03}", self.nanosecond / 1_000_000)
            } else {
                format!(".{:06}", self.nanosecond / 1000)
            });
        }
        s
    }
    fn invalid() -> ImageError {
        ImageError::Invalid {
            reason: "MySQL DATETIME calendar or microsecond precision",
        }
    }
}
