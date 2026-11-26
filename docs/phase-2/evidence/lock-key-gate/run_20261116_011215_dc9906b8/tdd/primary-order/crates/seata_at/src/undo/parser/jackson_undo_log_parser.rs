use super::{
    bounded_output::BoundedOutput, strict_json::StrictJson, value_format::ValueFormat,
    UndoCodecError, UndoCodecLimits, UndoLogDocument,
};
use super::{prepared_format::PreparedFormat, MySqlJdbcProfile};
use crate::SqlUndoLog;
use serde_json::{Map, Value};

const BRANCH: &str = "org.apache.seata.rm.datasource.undo.BranchUndoLog";
const SQL: &str = "org.apache.seata.rm.datasource.undo.SQLUndoLog";
const RECORDS: &str = "org.apache.seata.rm.datasource.sql.struct.TableRecords";
const EMPTY: &str = "org.apache.seata.rm.datasource.sql.struct.TableRecords$EmptyTableRecords";
const ROW: &str = "org.apache.seata.rm.datasource.sql.struct.Row";
const FIELD: &str = "org.apache.seata.rm.datasource.sql.struct.Field";

/// 原Jackson对象图的有界编解码；不执行Java类型加载，不把原DTO视作已验证事务镜像。
/// 对应 Java: org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser（部分实现）。
#[derive(Debug, Default, Clone)]
pub struct JacksonUndoLogParser {
    limits: UndoCodecLimits,
}
impl JacksonUndoLogParser {
    /// 参数为原XID/已登记号、事务内日志及明确JDBC配置；返回Java对象图字节，不证明登记/持久化或提交。
    pub fn encode_prepared(
        &self,
        xid: &str,
        branch_id: i64,
        logs: &[SqlUndoLog],
        profile: MySqlJdbcProfile,
    ) -> Result<Vec<u8>, UndoCodecError> {
        let value = PreparedFormat::build(xid, branch_id, logs, profile, self.limits)?;
        Self::branch(&value)?;
        StrictJson::validate(&value, self.limits)?;
        BoundedOutput::encode(&value, self.limits.encoded)
    }
    /// 参数为已验证且不超过硬上限的预算；返回独立文档parser，不访问网络或数据库。
    pub fn new(limits: UndoCodecLimits) -> Self {
        Self { limits }
    }
    /// 无参数；返回原parser默认的两字节内容；不同于普通缺省分支再编码结果。
    pub const fn default_content(&self) -> &'static [u8] {
        b"{}"
    }
    /// 参数为完整原JSON字节；返回可空分支文档，拒绝重复键/未知类/越界及尾随内容。
    pub fn decode(&self, bytes: &[u8]) -> Result<Option<UndoLogDocument>, UndoCodecError> {
        let mut value = StrictJson::parse(bytes, self.limits)?;
        if value.is_null() {
            return Ok(None);
        }
        if bytes == self.default_content() {
            value = serde_json::json!({"@class":BRANCH,"xid":null,"branchId":0,"sqlUndoLogs":null});
            // 默认Bean的结构同样计预算；输入仍是原两字节，不拿规范输出长度替换输入限额。
            StrictJson::validate(&value, self.limits)?;
        }
        Self::branch(&value)?;
        Ok(Some(UndoLogDocument { root: value }))
    }
    /// 参数为已验证文档或原null；返回确定键序的有界JSON，保留原集合/类标签和数值词法。
    pub fn encode(&self, document: Option<&UndoLogDocument>) -> Result<Vec<u8>, UndoCodecError> {
        let value = document.map_or(&Value::Null, |document| &document.root);
        // 文档可能来自预算较大的parser；本次编码先重新核结构/字节，写入器再限制最终输出。
        StrictJson::validate(value, self.limits)?;
        BoundedOutput::encode(value, self.limits.encoded)
    }
    fn object<'a>(
        value: &'a Value,
        classes: &[&str],
        keys: &[&str],
    ) -> Result<&'a Map<String, Value>, UndoCodecError> {
        let object = value
            .as_object()
            .ok_or(UndoCodecError::Shape { reason: "object" })?;
        if !object
            .get("@class")
            .and_then(Value::as_str)
            .is_some_and(|class| classes.contains(&class))
        {
            return Err(UndoCodecError::Unsupported {
                kind: "object class",
            });
        }
        if object
            .keys()
            .any(|key| key != "@class" && !keys.contains(&key.as_str()))
        {
            return Err(UndoCodecError::Unsupported {
                kind: "object member",
            });
        }
        Ok(object)
    }
    fn nullable_text(value: Option<&Value>) -> Result<(), UndoCodecError> {
        if value.is_none_or(|value| value.is_null() || value.is_string()) {
            Ok(())
        } else {
            Err(UndoCodecError::Shape {
                reason: "nullable text",
            })
        }
    }
    fn collection(
        value: Option<&Value>,
        maximum: usize,
    ) -> Result<Option<&[Value]>, UndoCodecError> {
        let Some(value) = value.filter(|v| !v.is_null()) else {
            return Ok(None);
        };
        let wrapper =
            value
                .as_array()
                .filter(|value| value.len() == 2)
                .ok_or(UndoCodecError::Shape {
                    reason: "typed collection",
                })?;
        if !matches!(
            wrapper[0].as_str(),
            Some("java.util.ArrayList" | "java.util.LinkedList")
        ) {
            return Err(UndoCodecError::Unsupported {
                kind: "collection class",
            });
        }
        let contents = wrapper[1].as_array().ok_or(UndoCodecError::Shape {
            reason: "collection contents",
        })?;
        if contents.len() > maximum {
            return Err(UndoCodecError::Budget {
                limit: "typed collection elements",
            });
        }
        Ok(Some(contents))
    }
    fn branch(value: &Value) -> Result<(), UndoCodecError> {
        let object = Self::object(value, &[BRANCH], &["xid", "branchId", "sqlUndoLogs"])?;
        Self::nullable_text(object.get("xid"))?;
        if let Some(id) = object.get("branchId").filter(|value| !value.is_null()) {
            ValueFormat::integer(id, i64::MIN, i64::MAX)?;
        }
        if let Some(logs) = Self::collection(object.get("sqlUndoLogs"), 128)? {
            for log in logs.iter().filter(|value| !value.is_null()) {
                Self::sql(log)?;
            }
        }
        Ok(())
    }
    fn sql(value: &Value) -> Result<(), UndoCodecError> {
        let object = Self::object(
            value,
            &[SQL],
            &["sqlType", "tableName", "beforeImage", "afterImage"],
        )?;
        Self::nullable_text(object.get("tableName"))?;
        if let Some(kind) = object.get("sqlType").filter(|value| !value.is_null()) {
            let name = kind.as_str().ok_or(UndoCodecError::Shape {
                reason: "SQLType label",
            })?;
            if !super::sql_type_labels::SQL_TYPE_LABELS.contains(&name) {
                return Err(UndoCodecError::Unsupported {
                    kind: "SQLType label",
                });
            }
        }
        for key in ["beforeImage", "afterImage"] {
            if let Some(value) = object.get(key).filter(|value| !value.is_null()) {
                Self::records(value)?;
            }
        }
        Ok(())
    }
    fn records(value: &Value) -> Result<(), UndoCodecError> {
        let object = Self::object(value, &[RECORDS, EMPTY], &["tableName", "rows"])?;
        Self::nullable_text(object.get("tableName"))?;
        if let Some(rows) = Self::collection(object.get("rows"), 1024)? {
            for row in rows.iter().filter(|value| !value.is_null()) {
                let object = Self::object(row, &[ROW], &["fields"])?;
                if let Some(fields) = Self::collection(object.get("fields"), 128)? {
                    for field in fields.iter().filter(|value| !value.is_null()) {
                        Self::field(field)?;
                    }
                }
            }
        }
        Ok(())
    }
    fn field(value: &Value) -> Result<(), UndoCodecError> {
        let object = Self::object(value, &[FIELD], &["name", "keyType", "type", "value"])?;
        Self::nullable_text(object.get("name"))?;
        if let Some(kind) = object.get("keyType").filter(|value| !value.is_null()) {
            if !matches!(kind.as_str(), Some("NULL" | "PRIMARY_KEY")) {
                return Err(UndoCodecError::Unsupported {
                    kind: "key type label",
                });
            }
        }
        if let Some(code) = object.get("type").filter(|value| !value.is_null()) {
            ValueFormat::integer(code, i32::MIN.into(), i32::MAX.into())?;
        }
        if let Some(value) = object.get("value") {
            ValueFormat::check(value)?;
        }
        Ok(())
    }
}
