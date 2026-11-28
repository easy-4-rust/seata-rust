use crate::DbError;
use seata_datasource::LocalDateTime;
use std::time::{SystemTime, UNIX_EPOCH};

/// 固定 UTC 客户端日历下的原保留期零点分界；一次请求只计算一次。
/// 对应 Java: RMHandlerAT#getLogCreated、DateUtil#getDateNowPlusDays（UTC 参考配置）。
pub(crate) struct UndoRetentionCutoff;
impl UndoRetentionCutoff {
    pub(crate) fn now(save_days: u16) -> Result<String, DbError> {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|v| i64::try_from(v.as_secs()).ok())
            .ok_or(DbError::Invalid {
                reason: "Undo retention UTC clock unavailable",
            })?;
        Self::at(seconds, save_days)
    }
    fn at(seconds: i64, save_days: u16) -> Result<String, DbError> {
        if save_days == 0 || save_days > i16::MAX as u16 {
            return Err(DbError::Invalid {
                reason: "normalized Undo retention days required",
            });
        }
        let days = seconds
            .div_euclid(86_400)
            .checked_sub(i64::from(save_days))
            .and_then(|v| v.checked_mul(86_400))
            .ok_or(DbError::Invalid {
                reason: "Undo retention calendar overflow",
            })?;
        Ok(LocalDateTime::from_epoch_seconds(days, 0)?.to_sql_string())
    }
}
#[cfg(test)]
mod tests {
    use super::UndoRetentionCutoff;
    #[test]
    fn cutoff_is_utc_midnight_not_rolling_twenty_four_hours() {
        for second in [0, 1, 86399] {
            assert_eq!(
                UndoRetentionCutoff::at(86400 * 20000 + second, 7)
                    .unwrap_or_else(|e| panic!("{e}")),
                "2024-09-27 00:00:00"
            );
        }
        assert!(UndoRetentionCutoff::at(0, 0).is_err());
        assert!(UndoRetentionCutoff::at(0, u16::MAX).is_err());
    }
}
