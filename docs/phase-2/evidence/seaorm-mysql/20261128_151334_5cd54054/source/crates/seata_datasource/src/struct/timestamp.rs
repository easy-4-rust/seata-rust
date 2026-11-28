use crate::{ImageError, LocalDateTime};

/// MySQL TIMESTAMP 的精确 UTC 时间点，保留微秒及独立于 DATETIME 的身份。
/// 对应 Java: java.sql.Timestamp（MySQL 1970–2038 物理范围）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamp {
    calendar_utc: LocalDateTime,
}

impl Timestamp {
    /// 参数为 UTC 纪元秒及秒内纳秒；返回合法 MySQL 时间点，拒绝零秒、溢出及亚微秒。
    pub fn from_epoch_seconds(seconds: i64, nanosecond: u32) -> Result<Self, ImageError> {
        if !(1..=2_147_483_647).contains(&seconds) {
            return Err(ImageError::Invalid {
                reason: "MySQL TIMESTAMP epoch range",
            });
        }
        Ok(Self {
            calendar_utc: LocalDateTime::from_epoch_seconds(seconds, nanosecond)?,
        })
    }

    /// 参数为已确认 UTC 会话返回的完整日历文本；返回独立时间点，不采用宿主时区。
    pub fn parse_sql_utc(text: &str) -> Result<Self, ImageError> {
        let calendar = LocalDateTime::parse_sql(text)?;
        Self::from_epoch_seconds(calendar.epoch_seconds(), calendar.nanosecond())
    }

    /// 无参数；返回该时间点的 UTC 纪元秒。
    pub fn epoch_seconds(&self) -> i64 {
        self.calendar_utc.epoch_seconds()
    }

    /// 无参数；返回已保证微秒对齐的秒内纳秒。
    pub const fn nanosecond(&self) -> u32 {
        self.calendar_utc.nanosecond()
    }

    /// 无参数；返回供 UTC 数据库会话绑定的完整日历文本，非零小数使用六位。
    pub fn to_sql_utc_string(&self) -> String {
        self.calendar_utc.to_sql_string()
    }

    /// 无参数；返回 Java Timestamp#toString 在 UTC JVM 下的锁键，始终保留秒及小数点。
    pub fn to_java_utc_string(&self) -> String {
        let full = self.calendar_utc.to_sql_string();
        if self.nanosecond() == 0 {
            format!("{full}.0")
        } else {
            full.trim_end_matches('0').to_owned()
        }
    }
}
