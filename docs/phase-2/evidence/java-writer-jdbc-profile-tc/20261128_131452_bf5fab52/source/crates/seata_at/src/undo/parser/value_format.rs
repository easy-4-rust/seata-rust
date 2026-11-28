use super::{dmdb_timestamp::DMDB_TIMESTAMP, UndoCodecError};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;

/// Jackson字段值的声明类型词汇与精确数值边界；不执行类加载或数据库转换。
/// 对应 Java: JacksonUndoLogParser内置值serializer/deserializer的Rust线格式检查扩展。
pub(crate) struct ValueFormat;
impl ValueFormat {
    pub(crate) fn check(value: &Value) -> Result<(), UndoCodecError> {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
            Value::String(text) => Self::text(text),
            Value::Array(items) if items.len() == 2 => {
                let class = items[0].as_str().ok_or(Self::shape("value type label"))?;
                let payload = &items[1];
                match class {
                    "java.lang.Boolean" if payload.is_boolean() => Ok(()),
                    "java.lang.Byte" => Self::integer(payload, i8::MIN.into(), i8::MAX.into()),
                    "java.lang.Short" => Self::integer(payload, i16::MIN.into(), i16::MAX.into()),
                    "java.lang.Integer" => Self::integer(payload, i32::MIN.into(), i32::MAX.into()),
                    "java.lang.Long" => Self::integer(payload, i64::MIN, i64::MAX),
                    "java.math.BigInteger" => {
                        let text = payload
                            .as_number()
                            .ok_or(Self::shape("BigInteger number"))?
                            .to_string();
                        if text
                            .bytes()
                            .enumerate()
                            .all(|(i, b)| b.is_ascii_digit() || i == 0 && b == b'-')
                        {
                            Ok(())
                        } else {
                            Err(Self::shape("BigInteger integer"))
                        }
                    }
                    "java.math.BigDecimal" => Self::decimal(payload),
                    "java.lang.Float" | "java.lang.Double" => {
                        if payload.is_number()
                            || matches!(payload.as_str(), Some("NaN" | "Infinity" | "-Infinity"))
                        {
                            Ok(())
                        } else {
                            Err(Self::shape("floating value"))
                        }
                    }
                    "[B" | "javax.sql.rowset.serial.SerialBlob" => Self::binary(payload),
                    "javax.sql.rowset.serial.SerialClob" | "java.lang.String" => {
                        Self::text(payload.as_str().ok_or(Self::shape("text value"))?)
                    }
                    "java.sql.Timestamp" | "java.time.LocalDateTime" | DMDB_TIMESTAMP => {
                        Self::timestamp(payload)
                    }
                    super::offset_date_time::OFFSET_DATE_TIME => {
                        let number = payload
                            .as_number()
                            .ok_or(Self::shape("OffsetDateTime number"))?;
                        super::OffsetDateTime::decode_number(number).map(|_| ())
                    }
                    "java.sql.Date" | "java.sql.Time" | "java.util.Date" => {
                        Self::integer(payload, i64::MIN, i64::MAX)
                    }
                    _ => Err(UndoCodecError::Unsupported {
                        kind: "field value class",
                    }),
                }
            }
            Value::Object(object)
                if object.get("@class").and_then(Value::as_str)
                    == Some("org.apache.seata.rm.datasource.sql.serial.SerialArray") =>
            {
                if object.keys().any(|key| {
                    !matches!(
                        key.as_str(),
                        "@class" | "baseType" | "baseTypeName" | "elements"
                    )
                }) {
                    return Err(Self::shape("SerialArray members"));
                }
                if let Some(kind) = object.get("baseType").filter(|value| !value.is_null()) {
                    Self::integer(kind, i32::MIN.into(), i32::MAX.into())?;
                }
                if let Some(name) = object.get("baseTypeName").filter(|value| !value.is_null()) {
                    Self::text(name.as_str().ok_or(Self::shape("SerialArray name"))?)?;
                }
                if let Some(elements) = object.get("elements").filter(|value| !value.is_null()) {
                    let array = elements
                        .as_array()
                        .ok_or(Self::shape("SerialArray elements"))?;
                    if array.len() > 1024 {
                        return Err(UndoCodecError::Budget {
                            limit: "SerialArray elements",
                        });
                    }
                    for element in array {
                        Self::check(element)?;
                    }
                }
                Ok(())
            }
            _ => Err(Self::shape("field value")),
        }
    }
    fn shape(reason: &'static str) -> UndoCodecError {
        UndoCodecError::Shape { reason }
    }
    fn decimal(value: &Value) -> Result<(), UndoCodecError> {
        let number = value
            .as_number()
            .ok_or(Self::shape("BigDecimal number"))?
            .to_string();
        Self::decimal_parts(&number).map(|_| ())
    }

    pub(super) fn decimal_parts(number: &str) -> Result<(&str, i64), UndoCodecError> {
        let (significand, exponent) = match number.split_once(['e', 'E']) {
            Some((significand, exponent)) => {
                let exponent = exponent
                    .parse::<i64>()
                    .map_err(|_| Self::shape("BigDecimal exponent"))?;
                // 原BigDecimal字符串构造器拒绝绝对值超过Integer.MAX_VALUE的指数。
                if !(-i64::from(i32::MAX)..=i64::from(i32::MAX)).contains(&exponent) {
                    return Err(Self::shape("BigDecimal exponent"));
                }
                (significand, exponent)
            }
            None => (number, 0),
        };
        let fractional = significand
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        let scale = i64::try_from(fractional)
            .ok()
            .and_then(|length| length.checked_sub(exponent));
        if scale.is_some_and(|scale| i32::try_from(scale).is_ok()) {
            Ok((significand, exponent))
        } else {
            Err(Self::shape("BigDecimal scale"))
        }
    }
    fn text(value: &str) -> Result<(), UndoCodecError> {
        if value.len() > 1_048_576 {
            Err(UndoCodecError::Budget {
                limit: "field text",
            })
        } else {
            Ok(())
        }
    }
    fn binary(value: &Value) -> Result<(), UndoCodecError> {
        let encoded = value.as_str().ok_or(Self::shape("binary base64"))?;
        if encoded.len() > 1_398_104 {
            return Err(UndoCodecError::Budget {
                limit: "field binary base64",
            });
        }
        let decoded = STANDARD
            .decode(encoded)
            .map_err(|_| Self::shape("binary base64"))?;
        if decoded.len() > 1_048_576 {
            return Err(UndoCodecError::Budget {
                limit: "field binary",
            });
        }
        Ok(())
    }
    pub(crate) fn integer(value: &Value, minimum: i64, maximum: i64) -> Result<(), UndoCodecError> {
        if value
            .as_i64()
            .is_some_and(|value| value >= minimum && value <= maximum)
        {
            Ok(())
        } else {
            Err(Self::shape("integer range"))
        }
    }
    fn timestamp(value: &Value) -> Result<(), UndoCodecError> {
        if let Some(parts) = value.as_array() {
            if parts.len() != 2 {
                return Err(Self::shape("timestamp parts"));
            }
            Self::integer(&parts[0], i64::MIN, i64::MAX)?;
            Self::integer(&parts[1], 0, 999_999_999)
        } else {
            Self::integer(value, i64::MIN, i64::MAX)
        }
    }
}
