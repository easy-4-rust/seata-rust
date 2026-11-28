use crate::{bounded_json_writer::BoundedJsonWriter, SeaOrmMySqlError};
use sea_orm::Value;
use seata_datasource::{
    DecimalValue, FieldValue, InternalTime, LocalDate, LocalDateTime, Timestamp,
};

/// 消耗 SeaORM 原值，映射到正式 MySQL AT 精确值域；不把 Debug/Display 用作通用绑定。
/// 对应 Java: 无直接对象；PreparedStatementProxy 参数保真的 Rust ORM 扩展。
pub(crate) struct SeaOrmMySqlValue;
impl SeaOrmMySqlValue {
    /// 参数为拥有式 ORM 原值；返回预算内精确值，未知类型和精度丢失显式拒绝。
    pub(crate) fn convert(value: Value) -> Result<FieldValue, SeaOrmMySqlError> {
        let value = match value {
            Value::Bool(v) => v.map(|v| FieldValue::Signed(i64::from(v))),
            Value::TinyInt(v) => v.map(|v| FieldValue::Signed(i64::from(v))),
            Value::SmallInt(v) => v.map(|v| FieldValue::Signed(i64::from(v))),
            Value::Int(v) => v.map(|v| FieldValue::Signed(i64::from(v))),
            Value::BigInt(v) => v.map(FieldValue::Signed),
            Value::TinyUnsigned(v) => v.map(|v| FieldValue::Unsigned(u64::from(v))),
            Value::SmallUnsigned(v) => v.map(|v| FieldValue::Unsigned(u64::from(v))),
            Value::Unsigned(v) => v.map(|v| FieldValue::Unsigned(u64::from(v))),
            Value::BigUnsigned(v) => v.map(FieldValue::Unsigned),
            Value::String(v) => v.map(|v| FieldValue::Text(*v)),
            Value::Char(v) => v.map(|v| FieldValue::Text(v.to_string())),
            Value::Bytes(v) => v.map(|v| FieldValue::Binary(*v)),
            Value::Json(v) => v
                .map(|v| {
                    if let Err(error) = Self::json_shape(&v) {
                        Self::discard_json(*v);
                        return Err(error);
                    }
                    let mut output = BoundedJsonWriter::new();
                    serde_json::to_writer(&mut output, &*v).map_err(|_| {
                        SeaOrmMySqlError::Input {
                            reason: "JSON parameter budget or encoding",
                        }
                    })?;
                    output.finish().map(FieldValue::Text)
                })
                .transpose()?,
            Value::BigDecimal(v) => v
                .map(|v| {
                    let digits = v.digits();
                    let scale = v.fractional_digit_count();
                    // 展开前拒绝巨大正负指数，避免 to_plain_string 分配不受限文本。
                    if scale > 30
                        || i128::from(digits).saturating_sub(i128::from(scale)).max(0)
                            + i128::from(scale).max(0)
                            > 65
                    {
                        return Err(SeaOrmMySqlError::Input {
                            reason: "DECIMAL precision/scale budget",
                        });
                    }
                    Ok(FieldValue::Decimal(DecimalValue::new(
                        &v.to_plain_string(),
                    )?))
                })
                .transpose()?,
            Value::ChronoDate(v) => v
                .map(|v| LocalDate::parse_sql(&v.to_string()).map(FieldValue::LocalDate))
                .transpose()?,
            Value::ChronoTime(v) => v
                .map(|v| InternalTime::parse_sql(&v.to_string()).map(FieldValue::InternalTime))
                .transpose()?,
            Value::ChronoDateTime(v) => v
                .map(|v| {
                    let raw = v.and_utc();
                    LocalDateTime::from_epoch_seconds(raw.timestamp(), raw.timestamp_subsec_nanos())
                        .map(FieldValue::LocalDateTime)
                })
                .transpose()?,
            Value::ChronoDateTimeUtc(v) => v
                .map(|v| {
                    Timestamp::from_epoch_seconds(v.timestamp(), v.timestamp_subsec_nanos())
                        .map(FieldValue::Timestamp)
                })
                .transpose()?,
            Value::ChronoDateTimeLocal(v) => v
                .map(|v| {
                    Timestamp::from_epoch_seconds(v.timestamp(), v.timestamp_subsec_nanos())
                        .map(FieldValue::Timestamp)
                })
                .transpose()?,
            Value::ChronoDateTimeWithTimeZone(v) => v
                .map(|v| {
                    Timestamp::from_epoch_seconds(v.timestamp(), v.timestamp_subsec_nanos())
                        .map(FieldValue::Timestamp)
                })
                .transpose()?,
            // Cargo 的 feature 统一可能添加其他 SeaQuery 值；未认证类型一律显式拒绝。
            _ => {
                return Err(SeaOrmMySqlError::Input {
                    reason: "unsupported lossless MySQL parameter type",
                })
            }
        }
        .unwrap_or(FieldValue::Null);
        if value.value_bytes() > 1_048_576 {
            return Err(SeaOrmMySqlError::Input {
                reason: "parameter value budget",
            });
        }
        Ok(value)
    }
    fn json_shape(value: &serde_json::Value) -> Result<(), SeaOrmMySqlError> {
        let mut pending = vec![(value, 0_usize)];
        let mut visited = 0_usize;
        while let Some((value, depth)) = pending.pop() {
            visited += 1;
            if depth > 64 || visited > 8192 {
                return Err(SeaOrmMySqlError::Input {
                    reason: "JSON depth/node budget",
                });
            }
            let width = match value {
                serde_json::Value::Array(v) => v.len(),
                serde_json::Value::Object(v) => v.len(),
                _ => 0,
            };
            if width > 8192_usize.saturating_sub(visited + pending.len()) {
                return Err(SeaOrmMySqlError::Input {
                    reason: "JSON node budget",
                });
            }
            match value {
                serde_json::Value::Array(v) => pending.extend(v.iter().map(|v| (v, depth + 1))),
                serde_json::Value::Object(v) => pending.extend(v.values().map(|v| (v, depth + 1))),
                _ => {}
            }
        }
        Ok(())
    }
    fn discard_json(value: serde_json::Value) {
        // 对拒绝的深树迭代释放，避免 serde_json::Value 的递归 Drop 造成栈溢出。
        let mut pending: Vec<Box<dyn Iterator<Item = serde_json::Value>>> =
            vec![Box::new(std::iter::once(value))];
        while let Some(current) = pending.last_mut() {
            match current.next() {
                Some(serde_json::Value::Array(v)) => pending.push(Box::new(v.into_iter())),
                Some(serde_json::Value::Object(v)) => {
                    pending.push(Box::new(v.into_iter().map(|(_, v)| v)))
                }
                Some(_) => {}
                None => {
                    pending.pop();
                }
            }
        }
    }
}
