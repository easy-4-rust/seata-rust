use super::{fastjson_wire::LONG, UndoCodecError};
use serde_json::Value;

pub(crate) const SERIAL_ARRAY: &str = "org.apache.seata.rm.datasource.sql.serial.SerialArray";

/// 支持原 Fastjson BIGINT 元素的 JDBC 数组，保留非空类型名和空元素。
/// 对应 Java: org.apache.seata.rm.datasource.sql.serial.SerialArray（BIGINT 子集）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialArray {
    base_type_name: String,
    elements: Vec<Option<i64>>,
}

impl SerialArray {
    /// 参数为原 JDBC 类型名及有序可空 BIGINT 元素，返回原数组对象。
    pub fn bigint(base_type_name: String, elements: Vec<Option<i64>>) -> Self {
        Self {
            base_type_name,
            elements,
        }
    }

    /// 无参数；返回原 JDBC BIGINT 编号。
    pub const fn base_type(&self) -> i32 {
        -5
    }

    /// 无参数；返回原类型名。
    pub fn base_type_name(&self) -> &str {
        &self.base_type_name
    }

    /// 无参数；返回有序数组元素，保留每个空值位置。
    pub fn elements(&self) -> &[Option<i64>] {
        &self.elements
    }

    pub(crate) fn from_wire(value: &Value) -> Result<Self, UndoCodecError> {
        let object = value.as_object().ok_or(UndoCodecError::Shape {
            reason: "SerialArray object",
        })?;
        if object.get("@type").and_then(Value::as_str) != Some(SERIAL_ARRAY)
            || object.get("baseType").and_then(Value::as_i64) != Some(-5)
            || object.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "@type" | "baseType" | "baseTypeName" | "elements"
                )
            })
        {
            return Err(UndoCodecError::Unsupported {
                kind: "Fastjson SerialArray class/base type/members",
            });
        }
        let base_type_name =
            object
                .get("baseTypeName")
                .and_then(Value::as_str)
                .ok_or(UndoCodecError::Shape {
                    reason: "SerialArray name",
                })?;
        let raw_elements =
            object
                .get("elements")
                .and_then(Value::as_array)
                .ok_or(UndoCodecError::Shape {
                    reason: "SerialArray elements",
                })?;
        let elements = raw_elements
            .iter()
            .map(|element| {
                if element.is_null() {
                    return Ok(None);
                }
                // 普通 Integer 与原 Long 标记都绑定为精确 BIGINT；拒绝其他类型和有损整数转换。
                let integer = element.get(LONG).unwrap_or(element);
                integer.as_i64().map(Some).ok_or(UndoCodecError::Shape {
                    reason: "SerialArray BIGINT element",
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::bigint(base_type_name.to_owned(), elements))
    }
}
