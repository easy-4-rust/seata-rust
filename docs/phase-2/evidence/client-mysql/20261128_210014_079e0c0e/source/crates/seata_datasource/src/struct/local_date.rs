use crate::{ImageError, LocalDateTime};

/// MySQL DATE 的精确 Gregorian 日历值，保留 1582 年历法缺口中的合法数据库日期。
/// 对应 Java: java.time.LocalDate（MySQL 1000–9999 已认证范围）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalDate {
    calendar: LocalDateTime,
}

impl LocalDate {
    /// 参数为年、月、日；返回合法 MySQL 日历日期，不使用宿主时区或 JDK 混合日历。
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, ImageError> {
        Ok(Self {
            calendar: LocalDateTime::new(year, month, day, 0, 0, 0, 0)?,
        })
    }

    /// 参数为完整 YYYY-MM-DD 文本；返回严格消费的日期，拒绝零日期和非法 Gregorian 日历。
    pub fn parse_sql(text: &str) -> Result<Self, ImageError> {
        if text.len() != 10 {
            return Err(ImageError::Invalid {
                reason: "MySQL DATE calendar shape",
            });
        }
        Ok(Self {
            calendar: LocalDateTime::parse_sql(&format!("{text} 00:00:00"))?,
        })
    }

    /// 无参数；返回不经纪元或时区转换的完整 SQL 日期文本。
    pub fn to_sql_string(&self) -> String {
        let mut text = self.calendar.to_sql_string();
        text.truncate(10);
        text
    }
}
