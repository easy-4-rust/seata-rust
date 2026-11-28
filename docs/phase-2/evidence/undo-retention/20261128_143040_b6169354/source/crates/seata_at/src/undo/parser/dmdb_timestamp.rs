use super::UndoCodecError;

const GREGORIAN_CUTOVER_UTC: i64 = -12_219_292_800;

pub(super) const DMDB_TIMESTAMP: &str = "dm.jdbc.driver.DmdbTimestamp";

/// UTC 宿主下的达梦时间戳，保留创建来源对对象相等性的影响。
/// 对应 Java: dm.jdbc.driver.DmdbTimestamp 的 Date 与 UTC ZonedDateTime 工厂。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmdbTimestamp {
    pub(super) epoch_seconds: i64,
    pub(super) nanos: u32,
    zoned: bool,
}

impl DmdbTimestamp {
    /// 参数为 Unix 毫秒，返回 UTC 宿主下由 java.util.Date 得到的无时区对象。
    /// 公历切换前的驱动日历映射返回不支持。
    pub fn from_date_utc(epoch_millis: i64) -> Result<Self, UndoCodecError> {
        let mut value = Self::from_zoned_utc(
            epoch_millis.div_euclid(1000),
            (epoch_millis.rem_euclid(1000) as u32) * 1_000_000,
        )?;
        value.zoned = false;
        Ok(value)
    }

    /// 参数为 UTC Unix 秒及秒内纳秒，返回原带 UTC 时区的对象。
    /// 校验纳秒、毫秒与固定 JDK 公历切换边界。
    pub fn from_zoned_utc(epoch_seconds: i64, nanos: u32) -> Result<Self, UndoCodecError> {
        if epoch_seconds < GREGORIAN_CUTOVER_UTC {
            // 驱动把 ZonedDateTime 的公历字段交给混合历法的 GregorianCalendar。
            return Err(UndoCodecError::Unsupported {
                kind: "DmdbTimestamp historical calendar",
            });
        }
        let millis = i128::from(epoch_seconds) * 1000 + i128::from(nanos / 1_000_000);
        if nanos >= 1_000_000_000 || i64::try_from(millis).is_err() {
            return Err(UndoCodecError::Shape {
                reason: "DmdbTimestamp UTC instant",
            });
        }
        Ok(Self {
            epoch_seconds,
            nanos,
            zoned: true,
        })
    }

    /// 参数为另一个时间戳；返回二者是否为同一瞬间，保留纳秒精度。
    /// 对应 Java: DataCompareUtils#isFieldEquals 在 Jackson 模式下对达梦时间值的比较。
    pub fn same_instant(&self, other: &Self) -> bool {
        (self.epoch_seconds, self.nanos) == (other.epoch_seconds, other.nanos)
    }
}
