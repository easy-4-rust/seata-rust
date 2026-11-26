use super::{FastjsonUndoField, UndoCodecError};
use serde_json::Value;

/// Fastjson 原 Undo 普通记录对象，可保留原可空表名。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.TableRecords。
#[derive(PartialEq, Eq)]
pub struct FastjsonUndoRecords {
    pub(crate) table: Option<String>,
    pub(crate) rows: Vec<Vec<FastjsonUndoField>>,
}

impl FastjsonUndoRecords {
    /// 参数为原表名和有序行及字段；返回文档记录对象。
    pub fn new(table: Option<String>, rows: Vec<Vec<FastjsonUndoField>>) -> Self {
        Self { table, rows }
    }

    /// 参数为原有序字段；将一行追加至待编码记录。
    pub fn push_row(&mut self, row: Vec<FastjsonUndoField>) {
        self.rows.push(row);
    }

    /// 无参数；返回原可空表名。
    pub fn table_name(&self) -> Option<&str> {
        self.table.as_deref()
    }

    /// 无参数；返回原有序记录行。
    pub fn rows(&self) -> &[Vec<FastjsonUndoField>] {
        &self.rows
    }

    pub(super) fn from_wire(image: &Value) -> Result<Self, UndoCodecError> {
        let shape = || UndoCodecError::Shape {
            reason: "Fastjson original records",
        };
        let object = image.as_object().ok_or_else(shape)?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "rows" | "tableName"))
        {
            return Err(shape());
        }
        let table = match object.get("tableName").filter(|value| !value.is_null()) {
            Some(value) => Some(value.as_str().ok_or_else(shape)?.to_owned()),
            None => None,
        };
        let raw = object
            .get("rows")
            .and_then(Value::as_array)
            .ok_or_else(shape)?;
        let rows = raw
            .iter()
            .map(|row| {
                let row = row.as_object().ok_or_else(shape)?;
                if row.len() != 1 {
                    return Err(shape());
                }
                let fields = row
                    .get("fields")
                    .and_then(Value::as_array)
                    .ok_or_else(shape)?;
                fields
                    .iter()
                    .map(FastjsonUndoField::from_wire)
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::new(table, rows))
    }
}
