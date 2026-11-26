use super::{
    fastjson_wire::{BINARY, DECIMAL, LONG},
    serial_array::SERIAL_ARRAY,
    FastjsonSqlUndoLog, FastjsonUndoField, UndoCodecError,
};

use serde_json::Value;
use std::fmt;

/// Fastjson 原分支对象的有界信封，保留缺省字段和原日志集合的可空性。
/// 对应 Java: org.apache.seata.rm.datasource.undo.BranchUndoLog。
pub struct FastjsonUndoBranch {
    pub(crate) root: Value,
    pub(crate) encoded_bytes: usize,
    pub(crate) normalized_bytes: usize,
}

impl FastjsonUndoBranch {
    /// 无参数；返回原可空 XID。
    pub fn xid(&self) -> Option<&str> {
        self.root.get("xid").and_then(Value::as_str)
    }

    /// 无参数；返回原 Java long 分支号，缺省为零。
    pub fn branch_id(&self) -> i64 {
        self.root
            .get("branchId")
            .and_then(Value::as_i64)
            .unwrap_or(0)
    }

    /// 无参数；返回原日志集合长度，缺省或原 null 返回 None。
    pub fn sql_undo_log_count(&self) -> Option<usize> {
        self.root
            .get("sqlUndoLogs")
            .and_then(Value::as_array)
            .map(Vec::len)
    }

    /// 无参数；将受支持的原 SQL 日志、普通记录及装箱值解码为有序对象。
    /// 对应 Java: BranchUndoLog#getSqlUndoLogs；缺省集合返回 None。
    pub fn decode_sql_undo_logs(&self) -> Result<Option<Vec<FastjsonSqlUndoLog>>, UndoCodecError> {
        self.root
            .get("sqlUndoLogs")
            .and_then(Value::as_array)
            .map(|logs| logs.iter().map(FastjsonSqlUndoLog::from_wire).collect())
            .transpose()
    }

    /// 参数为日志下标、是否读取后镜像、行下标和列下标，返回受支持的 Java 值类型。
    /// 空引用、越界及不支持的值返回 None。对应 Java: Field#getValue 后的装箱类型。
    pub fn field_value_class(
        &self,
        log: usize,
        after: bool,
        row: usize,
        column: usize,
    ) -> Option<&'static str> {
        let value = self.field(log, after, row, column)?.get("value")?;
        match value {
            Value::Bool(_) => Some("java.lang.Boolean"),
            Value::String(_) => Some("java.lang.String"),
            Value::Number(number) if number.to_string().contains(['.', 'e', 'E']) => {
                Some("java.math.BigDecimal")
            }
            Value::Number(number)
                if number
                    .as_i64()
                    .is_some_and(|number| i32::try_from(number).is_ok()) =>
            {
                Some("java.lang.Integer")
            }
            Value::Number(number) if number.is_i64() => Some("java.lang.Long"),
            Value::Number(_) => Some("java.math.BigInteger"),
            Value::Object(object) if object.contains_key(LONG) => {
                Some(if object.get(LONG).and_then(Value::as_i64).is_some() {
                    "java.lang.Long"
                } else {
                    "java.math.BigInteger"
                })
            }
            Value::Object(object) if object.contains_key(DECIMAL) => Some("java.math.BigDecimal"),
            Value::Object(object) if object.contains_key(BINARY) => Some("[B"),
            Value::Object(object)
                if object.get("@type").and_then(Value::as_str) == Some(SERIAL_ARRAY) =>
            {
                Some(SERIAL_ARRAY)
            }
            _ => None,
        }
    }

    /// 参数为日志下标、镜像方向及行列下标；返回支持的原数组字段。
    /// 对应 Java: 解析后 SQLUndoLog 镜像中的 SerialArray 字段；索引缺失或非数组返回 None。
    pub fn serial_array_field(
        &self,
        log: usize,
        after: bool,
        row: usize,
        column: usize,
    ) -> Result<Option<FastjsonUndoField>, UndoCodecError> {
        let Some(field) = self.field(log, after, row, column) else {
            return Ok(None);
        };
        let Some(value) = field
            .get("value")
            .filter(|value| value.get("@type").and_then(Value::as_str) == Some(SERIAL_ARRAY))
        else {
            return Ok(None);
        };
        let _ = value;
        FastjsonUndoField::from_wire(field).map(Some)
    }

    fn field(&self, log: usize, after: bool, row: usize, column: usize) -> Option<&Value> {
        let image = if after { "afterImage" } else { "beforeImage" };
        self.root
            .get("sqlUndoLogs")?
            .get(log)?
            .get(image)?
            .get("rows")?
            .get(row)?
            .get("fields")?
            .get(column)
    }
}

impl fmt::Debug for FastjsonUndoBranch {
    fn fmt(&self, format: &mut fmt::Formatter<'_>) -> fmt::Result {
        format
            .debug_struct("FastjsonUndoBranch")
            .field("has_xid", &self.xid().is_some())
            .field("log_count", &self.sql_undo_log_count())
            .finish_non_exhaustive()
    }
}
