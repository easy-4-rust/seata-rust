use super::{
    fastjson_wire::{BINARY, DECIMAL, LONG},
    restored_format::RestoredFormat,
    serial_array::SERIAL_ARRAY,
    SerialArray, UndoCodecError,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use seata_datasource::DecimalValue;
use serde_json::{Number, Value};

/// 支持 Fastjson 原 Undo 编码的装箱值。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field#getValue。
#[derive(Clone, PartialEq, Eq)]
pub enum FastjsonUndoFieldValue {
    /// 空引用。
    Null,
    /// Java Boolean。
    Boolean(bool),
    /// Java Integer。
    Integer(i32),
    /// Java Long。
    Long(i64),
    /// MySQL 无符号整数对应的 Java BigInteger 输入。
    Unsigned(u64),
    /// Java BigDecimal。
    Decimal(DecimalValue),
    /// Java String。
    Text(String),
    /// Java byte[]。
    Binary(Vec<u8>),
    /// 支持的 JDBC SerialArray 对象。
    SerialArray(SerialArray),
}

impl FastjsonUndoFieldValue {
    pub(super) fn from_wire(value: &Value) -> Result<Self, UndoCodecError> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Boolean(*value)),
            Value::String(value) => Ok(Self::Text(value.clone())),
            Value::Number(value) => Self::number(value, None),
            Value::Object(object)
                if object.get("@type").and_then(Value::as_str) == Some(SERIAL_ARRAY) =>
            {
                Ok(Self::SerialArray(SerialArray::from_wire(value)?))
            }
            Value::Object(value) if value.len() == 1 => {
                let (key, literal) = value.iter().next().ok_or(Self::unsupported())?;
                match key.as_str() {
                    LONG | DECIMAL => {
                        Self::number(literal.as_number().ok_or(Self::unsupported())?, Some(key))
                    }
                    BINARY => {
                        let bytes = STANDARD
                            .decode(literal.as_str().ok_or(Self::unsupported())?)
                            .map_err(|_| Self::unsupported())?;
                        Ok(Self::Binary(bytes))
                    }
                    _ => Err(Self::unsupported()),
                }
            }
            _ => Err(Self::unsupported()),
        }
    }

    fn number(value: &Number, tag: Option<&str>) -> Result<Self, UndoCodecError> {
        let text = value.to_string();
        if tag == Some(DECIMAL) || text.contains(['.', 'e', 'E']) {
            return Ok(Self::Decimal(RestoredFormat::decimal(&text)?));
        }
        if let Some(integer) = value.as_i64() {
            if tag != Some(LONG) {
                if let Ok(small) = i32::try_from(integer) {
                    return Ok(Self::Integer(small));
                }
            }
            return Ok(Self::Long(integer));
        }
        value
            .as_u64()
            .map(Self::Unsigned)
            .ok_or(Self::unsupported())
    }

    fn unsupported() -> UndoCodecError {
        UndoCodecError::Unsupported {
            kind: "Fastjson original field value",
        }
    }
}
