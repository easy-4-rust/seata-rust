use super::{FastjsonUndoRecords, UndoCodecError};
use crate::SqlType;
use serde_json::Value;

/// Fastjson 原 SQL 日志对象，保留原始前后镜像。
/// 对应 Java: org.apache.seata.rm.datasource.undo.SQLUndoLog。
pub struct FastjsonSqlUndoLog {
    pub(crate) sql_type: SqlType,
    pub(crate) table: Option<String>,
    pub(crate) before: FastjsonUndoRecords,
    pub(crate) after: FastjsonUndoRecords,
}

impl FastjsonSqlUndoLog {
    /// 参数为 SQL 类型、日志表名和原前后镜像；返回独立待编码日志对象。
    pub fn new(
        sql_type: SqlType,
        table: Option<String>,
        before: FastjsonUndoRecords,
        after: FastjsonUndoRecords,
    ) -> Self {
        Self {
            sql_type,
            table,
            before,
            after,
        }
    }

    /// 无参数；返回原 SQL 类型。
    pub const fn sql_type(&self) -> SqlType {
        self.sql_type
    }

    /// 无参数；返回原 SQL 日志的可空表名。
    pub fn table_name(&self) -> Option<&str> {
        self.table.as_deref()
    }

    /// 无参数；返回原前镜像。
    pub const fn before_image(&self) -> &FastjsonUndoRecords {
        &self.before
    }

    /// 无参数；返回原后镜像。
    pub const fn after_image(&self) -> &FastjsonUndoRecords {
        &self.after
    }

    pub(super) fn from_wire(log: &Value) -> Result<Self, UndoCodecError> {
        let shape = || UndoCodecError::Shape {
            reason: "Fastjson original SQL log",
        };
        let object = log.as_object().ok_or_else(shape)?;
        if object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "sqlType" | "tableName" | "beforeImage" | "afterImage"
            )
        }) {
            return Err(shape());
        }
        let sql_type = match object.get("sqlType").and_then(Value::as_str) {
            Some("INSERT") => SqlType::Insert,
            Some("UPDATE") => SqlType::Update,
            Some("DELETE") => SqlType::Delete,
            _ => return Err(shape()),
        };
        let table = match object.get("tableName").filter(|value| !value.is_null()) {
            Some(value) => Some(value.as_str().ok_or_else(shape)?.to_owned()),
            None => None,
        };
        let before = FastjsonUndoRecords::from_wire(object.get("beforeImage").ok_or_else(shape)?)?;
        let after = FastjsonUndoRecords::from_wire(object.get("afterImage").ok_or_else(shape)?)?;
        Ok(Self::new(sql_type, table, before, after))
    }
}
