use crate::RbatisMySqlError;
use bigdecimal::BigDecimal;
use chrono::{Datelike, Timelike};
use rbs::Value;
use seata_datasource::{
    DecimalValue, FieldValue, InternalTime, LocalDate, LocalDateTime, Timestamp,
};
use std::str::FromStr;
/// 原序 RBS 参数的精确转换；先检查所有形状，再消耗已确认浅层的值。
/// 对应 Java: 无直接对象；PreparedStatementProxy 的 Rust 参数域扩展。
pub(crate) struct RbatisMySqlBindings;
impl RbatisMySqlBindings {
    /// 参数为拥有式原值和精确数量；返回有界且不内插 SQL 的绑定。
    pub(crate) fn convert(
        values: Vec<Value>,
        expected: usize,
    ) -> Result<Vec<FieldValue>, RbatisMySqlError> {
        if let Err(error) = Self::shape(&values, expected) {
            Self::discard(values);
            return Err(error);
        }
        let mut result = Vec::with_capacity(values.len());
        let mut total = 0_usize;
        for value in values {
            let converted = Self::value(value)?;
            total = total
                .checked_add(converted.value_bytes())
                .ok_or(Self::invalid("parameter byte overflow"))?;
            if total > 8_388_608 {
                return Err(Self::invalid("parameter total budget"));
            }
            result.push(converted);
        }
        Ok(result)
    }
    fn shape(values: &[Value], expected: usize) -> Result<(), RbatisMySqlError> {
        if values.len() != expected || values.len() > 1024 {
            return Err(Self::invalid("exact bounded parameter count required"));
        }
        for value in values {
            let bytes = match value {
                Value::Null
                | Value::Bool(_)
                | Value::I32(_)
                | Value::I64(_)
                | Value::U32(_)
                | Value::U64(_) => 8,
                Value::String(v) => v.len(),
                Value::Binary(v) => v.len(),
                Value::Ext(tag, inner) => match (*tag, inner.as_ref()) {
                    ("Decimal" | "Json" | "Date" | "Time" | "DateTime", Value::String(v)) => {
                        v.len()
                    }
                    ("Timestamp", Value::I64(_)) => 8,
                    _ => return Err(Self::invalid("unsupported extension shape")),
                },
                _ => return Err(Self::invalid("unsupported lossless parameter type")),
            };
            if bytes > 1_048_576 {
                return Err(Self::invalid("parameter cell budget"));
            }
        }
        Ok(())
    }
    fn value(value: Value) -> Result<FieldValue, RbatisMySqlError> {
        Ok(match value {
            Value::Null => FieldValue::Null,
            Value::Bool(v) => FieldValue::Signed(i64::from(v)),
            Value::I32(v) => FieldValue::Signed(i64::from(v)),
            Value::I64(v) => FieldValue::Signed(v),
            Value::U32(v) => FieldValue::Unsigned(u64::from(v)),
            Value::U64(v) => FieldValue::Unsigned(v),
            Value::String(v) => FieldValue::Text(v),
            Value::Binary(v) => FieldValue::Binary(v),
            Value::Ext(tag, inner) => match (*inner, tag) {
                (Value::String(text), "Decimal") => {
                    if text.len() > 256 {
                        return Err(Self::invalid("DECIMAL source budget"));
                    }
                    let value = BigDecimal::from_str(&text)
                        .map_err(|_| Self::invalid("invalid exact DECIMAL"))?;
                    let scale = value.fractional_digit_count();
                    let digits = value.digits();
                    if scale > 30
                        || i128::from(digits).saturating_sub(i128::from(scale)).max(0)
                            + i128::from(scale).max(0)
                            > 65
                    {
                        return Err(Self::invalid("DECIMAL precision/scale budget"));
                    }
                    FieldValue::Decimal(DecimalValue::new(&value.to_plain_string())?)
                }
                (Value::String(text), "Json") => {
                    serde_json::from_str::<serde::de::IgnoredAny>(&text)
                        .map_err(|_| Self::invalid("invalid bounded JSON text"))?;
                    FieldValue::Text(text)
                }
                (Value::String(text), "Date") => {
                    FieldValue::LocalDate(LocalDate::parse_sql(&text)?)
                }
                (Value::String(text), "Time") => {
                    FieldValue::InternalTime(InternalTime::parse_sql(&Self::microseconds(text)?)?)
                }
                (Value::String(text), "DateTime") => {
                    // 原 rbdc MySQL 编码器保留日历字段，不按输入 offset 平移。
                    let value = if text.as_bytes().get(10) == Some(&b'T') {
                        let value = chrono::DateTime::parse_from_rfc3339(&text)
                            .map_err(|_| Self::invalid("invalid native DateTime"))?;
                        LocalDateTime::new(
                            value.year(),
                            value.month() as u8,
                            value.day() as u8,
                            value.hour() as u8,
                            value.minute() as u8,
                            value.second() as u8,
                            value.nanosecond(),
                        )?
                    } else {
                        LocalDateTime::parse_sql(&Self::microseconds(text)?)?
                    };
                    FieldValue::LocalDateTime(value)
                }
                (Value::I64(milliseconds), "Timestamp") => {
                    FieldValue::Timestamp(Timestamp::from_epoch_seconds(
                        milliseconds.div_euclid(1000),
                        u32::try_from(milliseconds.rem_euclid(1000))
                            .map_err(|_| Self::invalid("timestamp fraction"))?
                            * 1_000_000,
                    )?)
                }
                (other, _) => {
                    Self::discard(vec![other]);
                    return Err(Self::invalid("unsupported extension value"));
                }
            },
            other => {
                Self::discard(vec![other]);
                return Err(Self::invalid("unsupported parameter value"));
            }
        })
    }
    fn microseconds(mut text: String) -> Result<String, RbatisMySqlError> {
        if let Some(dot) = text.find('.') {
            let fraction = &text[dot + 1..];
            if fraction.len() > 6 {
                if fraction.len() > 9
                    || !fraction.bytes().all(|b| b.is_ascii_digit())
                    || fraction.as_bytes()[6..].iter().any(|b| *b != b'0')
                {
                    return Err(Self::invalid("sub-microsecond calendar value"));
                }
                text.truncate(dot + 7);
            }
        }
        Ok(text)
    }
    /// 参数为任意深度拥有式容器；逐个拆解并释放，拒绝路径不递归析构。
    pub(crate) fn discard(mut values: Vec<Value>) {
        while let Some(value) = values.pop() {
            match value {
                Value::Array(mut children) => values.append(&mut children),
                Value::Map(map) => {
                    for (key, value) in map.0 {
                        values.push(key);
                        values.push(value);
                    }
                }
                Value::Ext(_, inner) => values.push(*inner),
                _ => {}
            }
        }
    }
    fn invalid(reason: &'static str) -> RbatisMySqlError {
        RbatisMySqlError::Input { reason }
    }
}
