use super::{
    fastjson_branch_encoder::FastjsonBranchEncoder,
    fastjson_wire::{FastjsonWire, BINARY, DECIMAL, LONG},
    restored_format::RestoredFormat,
    strict_json::StrictJson,
    FastjsonSqlUndoLog, FastjsonUndoBranch, FastjsonUndoDocument, MySqlJdbcProfile, PreparedFormat,
    SerialArray, UndoCodecError, UndoCodecLimits, UndoLogDocument,
};
use crate::SqlUndoLog;
use seata_datasource::{ColumnMeta, Field, TableMeta};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(super) const BRANCH: &str = "org.apache.seata.rm.datasource.undo.BranchUndoLog";
const SQL: &str = "org.apache.seata.rm.datasource.undo.SQLUndoLog";
const RECORDS: &str = "org.apache.seata.rm.datasource.sql.struct.TableRecords";
const EMPTY: &str = "org.apache.seata.rm.datasource.sql.struct.TableRecords$EmptyTableRecords";
const ROW: &str = "org.apache.seata.rm.datasource.sql.struct.Row";
const FIELD: &str = "org.apache.seata.rm.datasource.sql.struct.Field";

/// 有界解码已知 Fastjson Undo 结构并绑定完整 MySQL 物理镜像，不实例化 Java 类。
/// 对应 Java: org.apache.seata.rm.datasource.undo.parser.FastjsonUndoLogParser 的恢复边界。
#[derive(Debug, Default, Clone)]
pub struct FastjsonUndoLogParser {
    limits: UndoCodecLimits,
}

impl FastjsonUndoLogParser {
    /// 参数为校验后的预算，返回独立的 Fastjson Undo 解码器。
    pub fn new(limits: UndoCodecLimits) -> Self {
        Self { limits }
    }

    /// 无参数；返回 Fastjson 原默认内容。
    /// 对应 Java: FastjsonUndoLogParser#getDefaultContent。
    pub const fn default_content(&self) -> &'static [u8] {
        b"{}"
    }

    /// 参数为原字节，返回可空且经过有界解析的原分支信封。
    /// 对应 Java: FastjsonUndoLogParser#decode 的分支及默认字段；日志元素在恢复预检中校验。
    pub fn decode_branch(
        &self,
        bytes: &[u8],
    ) -> Result<Option<FastjsonUndoBranch>, UndoCodecError> {
        let (root, normalized_bytes) = FastjsonWire::parse(bytes, self.limits)?;
        if root.is_null() {
            return Ok(None);
        }
        let object = root
            .as_object()
            .ok_or(Self::bad("Fastjson branch object"))?;
        if object
            .get("@type")
            .is_some_and(|tag| tag.as_str() != Some(BRANCH))
        {
            return Err(Self::unsupported("Fastjson branch class"));
        }
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "@type" | "branchId" | "sqlUndoLogs" | "xid"))
        {
            return Err(Self::unsupported("Fastjson branch member"));
        }
        if object
            .get("xid")
            .is_some_and(|value| !value.is_null() && !value.is_string())
            || object
                .get("branchId")
                .is_some_and(|value| !value.is_null() && value.as_i64().is_none())
            || object
                .get("sqlUndoLogs")
                .is_some_and(|value| !value.is_null() && !value.is_array())
        {
            return Err(Self::bad("Fastjson branch envelope fields"));
        }
        Ok(Some(FastjsonUndoBranch {
            root,
            encoded_bytes: bytes.len(),
            normalized_bytes,
        }))
    }

    /// 参数为原分支字段与有序日志对象，返回带 Java 动态值类型的 Fastjson 字节。
    /// 对应 Java: FastjsonUndoLogParser#encode；物理镜像与事务身份由恢复预检另行校验。
    pub fn encode_branch(
        &self,
        xid: Option<&str>,
        branch_id: i64,
        logs: Option<&[FastjsonSqlUndoLog]>,
    ) -> Result<Vec<u8>, UndoCodecError> {
        FastjsonBranchEncoder::encode(xid, branch_id, logs, self.limits)
    }

    /// 参数为原数组；返回 Fastjson 直接序列化形式，小 BIGINT 元素使用普通数值。
    /// 对应 Java: 原 SerialArraySerializer 的独立 JSON 调用。
    pub fn encode_serial_array(&self, array: &SerialArray) -> Result<Vec<u8>, UndoCodecError> {
        FastjsonBranchEncoder::encode_array(array, self.limits)
    }

    /// 参数为直接编码的有界原数组；返回原类型名及可空 BIGINT 元素。
    /// 对应 Java: FastjsonUndoLogParser.SerialArrayDeserializer。
    pub fn decode_serial_array(&self, bytes: &[u8]) -> Result<SerialArray, UndoCodecError> {
        let (value, _) = FastjsonWire::parse(bytes, self.limits)?;
        SerialArray::from_wire(&value)
    }

    /// 参数为原载荷字节、权威分支身份、物理元数据及 JDBC 配置，返回完整有序的恢复镜像。
    /// 对应 Java: FastjsonUndoLogParser#decode 与 AbstractUndoLogManager 的物理元数据恢复步骤。
    pub fn restore_prepared(
        &self,
        bytes: &[u8],
        xid: &str,
        branch_id: i64,
        tables: &[TableMeta],
        profile: MySqlJdbcProfile,
    ) -> Result<Vec<SqlUndoLog>, UndoCodecError> {
        let document = self.decode_for_restore(bytes, xid, branch_id)?;
        self.restore_document(&document, xid, branch_id, tables, profile)
    }

    /// 参数为原载荷和权威分支身份，返回已核对身份、形状及表名的只读文档。
    /// 对应 Java: FastjsonUndoLogParser#decode 与 AbstractUndoLogManager#undo 的元数据读取前边界。
    pub fn decode_for_restore(
        &self,
        bytes: &[u8],
        xid: &str,
        branch_id: i64,
    ) -> Result<FastjsonUndoDocument, UndoCodecError> {
        let FastjsonUndoBranch {
            mut root,
            encoded_bytes,
            normalized_bytes,
        } = self
            .decode_branch(bytes)?
            .ok_or(Self::bad("Fastjson null original branch"))?;
        if root.get("@type").and_then(Value::as_str) != Some(BRANCH) {
            return Err(Self::unsupported("Fastjson branch class"));
        }
        let branch = Self::object(&mut root, BRANCH, &["branchId", "sqlUndoLogs", "xid"])?;
        Self::collection(branch, "sqlUndoLogs", 128, Self::sql)?;
        StrictJson::validate(&root, self.limits)?;
        let document = UndoLogDocument {
            root,
            encoded_bytes,
        };
        let logs = RestoredFormat::logs_for_branch(&document, xid, branch_id)?;
        let mut seen = HashSet::new();
        let mut tables = Vec::new();
        for log in logs {
            let name = log["tableName"]
                .as_str()
                .ok_or(Self::bad("Fastjson table name"))?;
            if seen.insert(name) {
                tables.push(name.to_owned());
            }
        }
        Ok(FastjsonUndoDocument {
            document,
            tables,
            encoded_bytes,
            normalized_bytes,
        })
    }

    /// 参数为已解析文档、权威分支身份及当前完整物理元数据，返回可供补偿器消费的有序日志。
    /// 对应 Java: AbstractUndoLogManager#undo 的元数据绑定边界。
    pub fn restore_document(
        &self,
        document: &FastjsonUndoDocument,
        xid: &str,
        branch_id: i64,
        tables: &[TableMeta],
        profile: MySqlJdbcProfile,
    ) -> Result<Vec<SqlUndoLog>, UndoCodecError> {
        if document.encoded_bytes > self.limits.encoded {
            return Err(UndoCodecError::Budget {
                limit: "encoded bytes",
            });
        }
        if document.normalized_bytes > self.limits.encoded {
            return Err(UndoCodecError::Budget {
                limit: "normalized Fastjson bytes",
            });
        }
        StrictJson::validate(&document.document.root, self.limits)?;
        RestoredFormat::build_with(
            &document.document,
            xid,
            branch_id,
            tables,
            profile,
            Self::restore_field,
        )
    }

    fn sql(value: &mut Value) -> Result<(), UndoCodecError> {
        let log = Self::object(
            value,
            SQL,
            &["beforeImage", "afterImage", "sqlType", "tableName"],
        )?;
        let name = log
            .get("tableName")
            .and_then(Value::as_str)
            .ok_or(Self::bad("Fastjson table name"))?;
        TableMeta::validate_identifier(name)
            .map_err(|_| Self::bad("Fastjson physical table identifier"))?;
        for image in ["beforeImage", "afterImage"] {
            if log
                .get(image)
                .and_then(|image| image.get("tableName"))
                .and_then(Value::as_str)
                != Some(name)
            {
                return Err(Self::bad("Fastjson image table identity"));
            }
        }
        let before = Self::image(
            log.get_mut("beforeImage")
                .ok_or(Self::bad("Fastjson before image"))?,
        )?;
        let after = Self::image(
            log.get_mut("afterImage")
                .ok_or(Self::bad("Fastjson after image"))?,
        )?;
        let shape_matches = match log.get("sqlType").and_then(Value::as_str) {
            Some("INSERT") => before == 0 && after > 0,
            Some("UPDATE") => before == after,
            Some("DELETE") => after == 0,
            _ => false,
        };
        if !shape_matches {
            return Err(Self::bad("Fastjson SQL type or image direction"));
        }
        Ok(())
    }

    fn image(image: &mut Value) -> Result<usize, UndoCodecError> {
        let class = if image.get("@type").and_then(Value::as_str) == Some(EMPTY) {
            EMPTY
        } else {
            RECORDS
        };
        let object = Self::object(image, class, &["tableName", "rows"])?;
        let count = Self::collection(object, "rows", 1024, Self::row)?;
        if class == EMPTY && count != 0 {
            return Err(Self::bad("Fastjson EmptyTableRecords has rows"));
        }
        Ok(count)
    }

    fn row(value: &mut Value) -> Result<(), UndoCodecError> {
        let row = Self::object(value, ROW, &["fields"])?;
        let mut seen = HashSet::new();
        let mut has_primary = false;
        Self::collection(row, "fields", 128, |field| {
            let members = Self::object(field, FIELD, &["name", "keyType", "type", "value"])?;
            let name = members
                .get("name")
                .and_then(Value::as_str)
                .ok_or(Self::bad("Fastjson column name"))?;
            TableMeta::validate_identifier(name)
                .map_err(|_| Self::bad("Fastjson physical column identifier"))?;
            if !seen.insert(name.to_ascii_lowercase()) {
                return Err(Self::bad("Fastjson duplicate physical column"));
            }
            let key = members.get("keyType").and_then(Value::as_str);
            if !matches!(key, Some("NULL" | "PRIMARY_KEY"))
                || members
                    .get("type")
                    .and_then(Value::as_i64)
                    .is_none_or(|code| i32::try_from(code).is_err())
            {
                return Err(Self::bad("Fastjson field key or JDBC type shape"));
            }
            let primary = key == Some("PRIMARY_KEY");
            let value = members.entry("value").or_insert(Value::Null);
            if primary && value.is_null() {
                return Err(Self::bad("Fastjson null primary field"));
            }
            has_primary |= primary;
            Self::scalar(value)?;
            Ok(())
        })?;
        if !has_primary {
            return Err(Self::bad("Fastjson missing primary field"));
        }
        Ok(())
    }

    fn object<'a>(
        value: &'a mut Value,
        class: &'static str,
        members: &[&str],
    ) -> Result<&'a mut Map<String, Value>, UndoCodecError> {
        let object = value.as_object_mut().ok_or(Self::bad("Fastjson object"))?;
        if object
            .remove("@type")
            .is_some_and(|tag| tag.as_str() != Some(class))
        {
            return Err(Self::unsupported("Fastjson object class"));
        }
        if object.keys().any(|key| !members.contains(&key.as_str())) {
            return Err(Self::unsupported("Fastjson object member"));
        }
        object.insert("@class".into(), Value::String(class.into()));
        Ok(object)
    }

    fn collection(
        object: &mut Map<String, Value>,
        name: &str,
        maximum: usize,
        mut normalize: impl FnMut(&mut Value) -> Result<(), UndoCodecError>,
    ) -> Result<usize, UndoCodecError> {
        let property = object
            .get_mut(name)
            .ok_or(Self::bad("Fastjson collection"))?;
        let values = property
            .as_array_mut()
            .ok_or(Self::bad("Fastjson collection array"))?;
        if values.len() > maximum {
            return Err(UndoCodecError::Budget {
                limit: "Fastjson collection elements",
            });
        }
        let count = values.len();
        for value in values {
            normalize(value)?;
        }
        let original = std::mem::take(property);
        *property = Value::Array(vec![Value::String("java.util.ArrayList".into()), original]);
        Ok(count)
    }

    fn restore_field(
        raw: &Value,
        column: &ColumnMeta,
        profile: MySqlJdbcProfile,
    ) -> Result<Field, UndoCodecError> {
        let raw_value = raw.get("value").ok_or(Self::bad("Fastjson field value"))?;
        let (literal, payload) = Self::scalar(raw_value)?;
        let field = RestoredFormat::native_field(payload, column)?;
        let mut expected = PreparedFormat::field(&field, profile)?;
        let expected_value = &expected["value"];
        let encoded_class = expected_value.as_array().and_then(|pair| pair[0].as_str());
        let matches = match encoded_class {
            Some("[B") => literal == Some(BINARY),
            Some("java.math.BigDecimal") => {
                literal == Some(DECIMAL)
                    || (literal.is_none()
                        && payload
                            .as_number()
                            .is_some_and(|number| number.to_string().contains(['.', 'e', 'E'])))
            }
            Some("java.lang.Long") => matches!(literal, None | Some(LONG)) && payload.is_number(),
            Some("java.math.BigInteger") => literal.is_none() && payload.is_number(),
            None => literal.is_none() && payload == expected_value,
            _ => false,
        };
        if !matches {
            return Err(Self::bad("Fastjson native scalar class"));
        }
        expected["value"] = raw_value.clone();
        if &expected != raw {
            return Err(Self::bad("Fastjson field name/key/JDBC type"));
        }
        Ok(field)
    }

    fn scalar(value: &Value) -> Result<(Option<&str>, &Value), UndoCodecError> {
        match value {
            Value::Object(object) if object.len() == 1 => {
                let (key, value) = object.iter().next().ok_or(Self::bad("Fastjson literal"))?;
                if !matches!(key.as_str(), BINARY | LONG | DECIMAL) {
                    return Err(Self::unsupported("Fastjson field object"));
                }
                Ok((Some(key.as_str()), value))
            }
            Value::Object(_) | Value::Array(_) => Err(Self::unsupported("Fastjson field object")),
            _ => Ok((None, value)),
        }
    }

    fn unsupported(kind: &'static str) -> UndoCodecError {
        UndoCodecError::Unsupported { kind }
    }

    fn bad(reason: &'static str) -> UndoCodecError {
        UndoCodecError::Shape { reason }
    }
}
