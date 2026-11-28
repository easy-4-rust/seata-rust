use crate::RbatisMySqlError;
use bigdecimal::BigDecimal;
use rbs::{value::map::ValueMap, Value};
use sqlx::{
    mysql::{types::MySqlTime, MySqlRow},
    Column, Row, TypeInfo, ValueRef,
};
/// 将核心已做长度预检的原 SQLx 行转为原序 RBS 映射；JSON/日历保持精确文本。
/// 对应 Java: 无直接对象；Rust rbatis ResultSet 的无损结果适配扩展。
pub(crate) struct RbatisMySqlRows;
impl RbatisMySqlRows {
    /// 参数为有界原生行；返回 rbatis 原生数组/字段映射，未知类型和重复标签明确拒绝。
    pub(crate) fn convert(rows: Vec<MySqlRow>) -> Result<Value, RbatisMySqlError> {
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let mut output = ValueMap::with_capacity(row.columns().len());
            for (index, column) in row.columns().iter().enumerate() {
                let value = if row
                    .try_get_raw(index)
                    .map_err(|_| Self::invalid())?
                    .is_null()
                {
                    Value::Null
                } else {
                    match column.type_info().name() {
                        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => {
                            Value::I64(row.try_get(index).map_err(|_| Self::invalid())?)
                        }
                        "TINYINT UNSIGNED" | "SMALLINT UNSIGNED" | "MEDIUMINT UNSIGNED"
                        | "INT UNSIGNED" | "BIGINT UNSIGNED" => {
                            Value::U64(row.try_get(index).map_err(|_| Self::invalid())?)
                        }
                        "DECIMAL" => Value::Ext(
                            "Decimal",
                            Box::new(Value::String(
                                row.try_get::<BigDecimal, _>(index)
                                    .map_err(|_| Self::invalid())?
                                    .to_string(),
                            )),
                        ),
                        "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" => {
                            Value::String(row.try_get(index).map_err(|_| Self::invalid())?)
                        }
                        "BINARY" | "VARBINARY" | "TINYBLOB" | "BLOB" | "MEDIUMBLOB"
                        | "LONGBLOB" => {
                            Value::Binary(row.try_get(index).map_err(|_| Self::invalid())?)
                        }
                        "JSON" => Value::String(
                            String::from_utf8(
                                row.try_get_unchecked::<Vec<u8>, _>(index)
                                    .map_err(|_| Self::invalid())?,
                            )
                            .map_err(|_| Self::invalid())?,
                        ),
                        "DATE" => Value::String(
                            row.try_get::<chrono::NaiveDate, _>(index)
                                .map_err(|_| Self::invalid())?
                                .format("%Y-%m-%d")
                                .to_string(),
                        ),
                        "TIME" => Value::String(Self::time_text(
                            row.try_get::<MySqlTime, _>(index)
                                .map_err(|_| Self::invalid())?,
                        )),
                        "DATETIME" => Value::String(
                            row.try_get::<chrono::NaiveDateTime, _>(index)
                                .map_err(|_| Self::invalid())?
                                .format("%Y-%m-%d %H:%M:%S%.6f")
                                .to_string(),
                        ),
                        "TIMESTAMP" => Value::String(
                            row.try_get::<chrono::DateTime<chrono::Utc>, _>(index)
                                .map_err(|_| Self::invalid())?
                                .format("%Y-%m-%dT%H:%M:%S%.6f+00:00")
                                .to_string(),
                        ),
                        _ => return Err(Self::invalid()),
                    }
                };
                if output
                    .insert(Value::String(column.name().into()), value)
                    .is_some()
                {
                    return Err(Self::invalid());
                }
            }
            result.push(Value::Map(output));
        }
        Ok(Value::Array(result))
    }
    fn time_text(value: MySqlTime) -> String {
        format!(
            "{}{:02}:{:02}:{:02}.{:06}",
            if value.sign().is_negative() { "-" } else { "" },
            value.hours(),
            value.minutes(),
            value.seconds(),
            value.microseconds()
        )
    }
    fn invalid() -> RbatisMySqlError {
        RbatisMySqlError::Input {
            reason: "unsupported or invalid exact native row",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RbatisMySqlRows;
    use sqlx::mysql::types::{MySqlTime, MySqlTimeSign};
    #[test]
    #[allow(clippy::unwrap_used)]
    fn time_format_preserves_both_signs_and_zero() {
        for (sign, text) in [
            (MySqlTimeSign::Positive, "01:02:03.123456"),
            (MySqlTimeSign::Negative, "-01:02:03.123456"),
        ] {
            assert_eq!(
                RbatisMySqlRows::time_text(MySqlTime::new(sign, 1, 2, 3, 123456).unwrap()),
                text
            );
        }
        assert_eq!(
            RbatisMySqlRows::time_text(MySqlTime::ZERO),
            "00:00:00.000000"
        );
        assert_eq!(
            RbatisMySqlRows::time_text(MySqlTime::MIN),
            "-838:59:59.000000"
        );
        assert_eq!(
            RbatisMySqlRows::time_text(MySqlTime::MAX),
            "838:59:59.000000"
        );
    }
}
