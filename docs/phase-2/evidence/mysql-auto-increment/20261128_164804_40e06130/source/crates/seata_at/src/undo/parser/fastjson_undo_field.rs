use super::{FastjsonUndoFieldValue, SerialArray, UndoCodecError};
use serde_json::Value;

/// Fastjson 原 Undo 字段，保留 JDBC 编号和主键标记。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field。
#[derive(Clone, PartialEq, Eq)]
pub struct FastjsonUndoField {
    pub(crate) name: String,
    pub(crate) jdbc_type: i32,
    pub(crate) primary: bool,
    pub(crate) value: FastjsonUndoFieldValue,
}

impl FastjsonUndoField {
    /// 参数为原列名、JDBC 类型、主键标记和装箱值；返回独立可编码字段。
    pub fn new(name: String, jdbc_type: i32, primary: bool, value: FastjsonUndoFieldValue) -> Self {
        Self {
            name,
            jdbc_type,
            primary,
            value,
        }
    }

    /// 无参数；返回原列名。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 无参数；返回原 JDBC 类型编号。
    pub const fn jdbc_type(&self) -> i32 {
        self.jdbc_type
    }

    /// 无参数；返回原主键标记。
    pub const fn is_primary(&self) -> bool {
        self.primary
    }

    /// 无参数；返回原装箱值。
    pub fn value(&self) -> &FastjsonUndoFieldValue {
        &self.value
    }

    /// 无参数；提取原值为 SerialArray 的字段，其余字段返回 None。
    pub fn into_serial_array(self) -> Option<SerialArray> {
        match self.value {
            FastjsonUndoFieldValue::SerialArray(array) => Some(array),
            _ => None,
        }
    }

    pub(super) fn from_wire(field: &Value) -> Result<Self, UndoCodecError> {
        let shape = || UndoCodecError::Shape {
            reason: "Fastjson original field metadata",
        };
        let values = field.as_object().ok_or_else(shape)?;
        if values
            .keys()
            .any(|key| !matches!(key.as_str(), "name" | "type" | "keyType" | "value"))
        {
            return Err(shape());
        }
        let name = values
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(shape)?;
        let jdbc = values
            .get("type")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(shape)?;
        let primary = match values.get("keyType").and_then(Value::as_str) {
            Some("PRIMARY_KEY") => true,
            Some("NULL") => false,
            _ => return Err(shape()),
        };
        let value = FastjsonUndoFieldValue::from_wire(values.get("value").unwrap_or(&Value::Null))?;
        Ok(Self::new(name.into(), jdbc, primary, value))
    }
}
