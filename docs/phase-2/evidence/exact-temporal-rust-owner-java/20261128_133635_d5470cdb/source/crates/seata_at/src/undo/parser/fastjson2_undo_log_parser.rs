use super::{
    bounded_output::BoundedOutput,
    jackson_undo_log_parser::{BRANCH, EMPTY},
    jsonb_reader::JsonbReader,
    jsonb_value::JsonbValue,
    jsonb_writer::JsonbWriter,
    prepared_format::PreparedFormat,
    restored_format::RestoredFormat,
    serial_array::SERIAL_ARRAY,
    sql_type_labels::SQL_TYPE_LABELS,
    Fastjson2UndoBranch, Fastjson2UndoDocument, FastjsonUndoFieldValue, JacksonSqlUndoLog,
    JacksonUndoField, JacksonUndoFieldValue, JacksonUndoRecords, MySqlJdbcProfile, SerialArray,
    UndoCodecError, UndoCodecLimits,
};
use crate::{SqlType, SqlUndoLog};
use seata_datasource::{ColumnMeta, ColumnType, Field, FieldValue, Row, TableMeta, TableRecords};
use std::collections::{BTreeMap, HashSet};

/// Fastjson2 JSONB 原 Undo 的有界对象编解码。
/// 对应 Java: org.apache.seata.rm.datasource.undo.parser.Fastjson2UndoLogParser。
#[derive(Debug, Default, Clone)]
pub struct Fastjson2UndoLogParser {
    limits: UndoCodecLimits,
}

impl Fastjson2UndoLogParser {
    /// 参数为检查过的预算，返回独立 JSONB 原对象编解码器。
    pub fn new(limits: UndoCodecLimits) -> Self {
        Self { limits }
    }

    /// 无参数；返回编码空原分支的 JSONB 默认内容。
    pub fn default_content(&self) -> Result<Vec<u8>, UndoCodecError> {
        self.encode_branch(None, 0, None)
    }

    /// 参数为原 XID、分支号及可空 SQL 日志，返回有界 JSONB 字节。
    pub fn encode_branch(
        &self,
        xid: Option<&str>,
        branch_id: i64,
        logs: Option<&[JacksonSqlUndoLog]>,
    ) -> Result<Vec<u8>, UndoCodecError> {
        let bytes = BoundedOutput::generate(self.limits.encoded, |out| {
            JsonbWriter::branch(out, xid, branch_id, logs)
        })?;
        self.decode_branch(&bytes)?;
        Ok(bytes)
    }

    /// 参数为完整原 JSONB 字节，返回原分支及受支持的装箱字段。
    /// 对应 Java: Fastjson2UndoLogParser#decode；对象投影不绑定数据库元数据。
    pub fn decode_branch(&self, bytes: &[u8]) -> Result<Fastjson2UndoBranch, UndoCodecError> {
        let mut root = Self::object(JsonbReader::parse(bytes, self.limits)?, Some(BRANCH))?;
        let xid = Self::text(root.remove("xid"))?;
        let branch_id = match root.remove("branchId") {
            None => 0,
            Some(JsonbValue::Long(id)) => id,
            _ => return Err(Self::shape("JSONB branch id")),
        };
        let logs = match root.remove("sqlUndoLogs") {
            None | Some(JsonbValue::Null) => None,
            Some(list) => Some(
                Self::list(Some(list), 128)?
                    .into_iter()
                    .map(Self::log)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        Self::end(&root)?;
        Ok(Fastjson2UndoBranch {
            xid,
            branch_id,
            logs,
        })
    }

    /// 参数为原 JSONB 字节、权威分支身份、完整 MySQL 元数据与 JDBC 配置，返回有序恢复日志。
    /// 对应 Java: Fastjson2UndoLogParser#decode 与 AbstractUndoLogManager#undo 的元数据绑定步骤。
    pub fn restore_prepared(
        &self,
        bytes: &[u8],
        xid: &str,
        branch_id: i64,
        tables: &[TableMeta],
        profile: MySqlJdbcProfile,
    ) -> Result<Vec<SqlUndoLog>, UndoCodecError> {
        self.decode_for_restore(bytes, xid, branch_id)?
            .bind(xid, branch_id, tables, profile)
    }

    /// 参数为原 JSONB 字节和锁定行的事务身份，返回已预检、可供元数据查询的只读文档。
    /// 对应 Java: AbstractUndoLogManager#undo 中解码后读取表元数据的步骤。
    pub fn decode_for_restore(
        &self,
        bytes: &[u8],
        xid: &str,
        branch_id: i64,
    ) -> Result<Fastjson2UndoDocument, UndoCodecError> {
        let branch = self.decode_branch(bytes)?;
        if xid.is_empty()
            || xid.len() > 128
            || branch_id <= 0
            || branch.xid() != Some(xid)
            || branch.branch_id() != branch_id
        {
            return Err(Self::shape("JSONB restored original branch identity"));
        }
        let logs = branch
            .logs()
            .filter(|logs| !logs.is_empty())
            .ok_or(Self::shape("JSONB restored nonempty log collection"))?;
        let mut seen = HashSet::new();
        let mut tables = Vec::new();
        for log in logs {
            let table = Self::preflight(log)?;
            if seen.insert(table) {
                tables.push(table.to_owned());
            }
        }
        Ok(Fastjson2UndoDocument { branch, tables })
    }

    pub(super) fn bind_logs(
        logs: &[JacksonSqlUndoLog],
        tables: &[TableMeta],
        profile: MySqlJdbcProfile,
    ) -> Result<Vec<SqlUndoLog>, UndoCodecError> {
        // 直接绑定原装箱值，避免重新序列化更大的中间文档。
        let mut total = 0;
        logs.iter()
            .map(|log| {
                let table = RestoredFormat::physical_table(
                    tables,
                    log.table_name()
                        .ok_or(Self::shape("JSONB physical table name"))?,
                )?;
                let kind = match log.sql_type() {
                    Some("INSERT") => SqlType::Insert,
                    Some("UPDATE") => SqlType::Update,
                    Some("DELETE") => SqlType::Delete,
                    _ => return Err(Self::shape("JSONB physical SQL type")),
                };
                let before = Self::bind_image(log.before_image(), table, profile)?;
                let after = Self::bind_image(log.after_image(), table, profile)?;
                RestoredFormat::checked_log(kind, before, after, &mut total)
            })
            .collect()
    }

    fn bind_image(
        image: Option<&JacksonUndoRecords>,
        table: &TableMeta,
        profile: MySqlJdbcProfile,
    ) -> Result<TableRecords, UndoCodecError> {
        let image = image.ok_or(Self::shape("JSONB physical image"))?;
        let mut records = TableRecords::new(table.clone());
        for row in image.rows() {
            if row.len() != table.columns().len() {
                return Err(Self::shape("JSONB complete physical columns"));
            }
            let fields = row
                .iter()
                .zip(table.columns())
                .map(|(field, column)| Self::bind_field(field, column, profile))
                .collect::<Result<Vec<_>, _>>()?;
            records
                .add(Row::new(table, fields).map_err(|_| Self::shape("JSONB physical row"))?)
                .map_err(|_| Self::shape("JSONB physical image row budget"))?;
        }
        Ok(records)
    }

    fn bind_field(
        raw: &JacksonUndoField,
        column: &ColumnMeta,
        profile: MySqlJdbcProfile,
    ) -> Result<Field, UndoCodecError> {
        use FastjsonUndoFieldValue as V;

        let (code, class) = PreparedFormat::physical_descriptor(column, profile)?;
        let error = Self::shape("JSONB field name/key/JDBC type/value class");
        if raw.name != column.name()
            || raw.primary != column.primary_key_ordinal().is_some()
            || raw.jdbc_type != code
            || !matches!(raw.value.java_class(), "null") && raw.value.java_class() != class
        {
            return Err(error);
        }
        let JacksonUndoFieldValue::Common(value) = raw.value() else {
            return Err(error);
        };
        let value = match value {
            V::Null => FieldValue::Null,
            V::Boolean(value) => FieldValue::Signed(i64::from(*value)),
            V::Integer(value) => Self::integer_field(i64::from(*value), column.kind())?,
            V::Long(value) => Self::integer_field(*value, column.kind())?,
            V::Unsigned(value) => FieldValue::Unsigned(*value),
            V::Decimal(value) => FieldValue::Decimal(value.clone()),
            V::Text(value) => FieldValue::Text(value.clone()),
            V::Binary(value) => FieldValue::Binary(value.clone()),
            V::SerialArray(_) => return Err(error),
        };
        Field::new(column, value).map_err(|_| Self::shape("JSONB physical value domain"))
    }

    fn integer_field(number: i64, kind: ColumnType) -> Result<FieldValue, UndoCodecError> {
        if kind == ColumnType::UnsignedInteger {
            return u64::try_from(number)
                .map(FieldValue::Unsigned)
                .map_err(|_| Self::shape("JSONB physical unsigned value"));
        }
        Ok(FieldValue::Signed(number))
    }

    fn preflight(log: &JacksonSqlUndoLog) -> Result<&str, UndoCodecError> {
        let table = log
            .table_name()
            .ok_or(Self::shape("JSONB physical table name"))?;
        TableMeta::validate_identifier(table)
            .map_err(|_| Self::shape("JSONB physical table identifier"))?;
        let before = Self::physical_image(log.before_image(), table)?;
        let after = Self::physical_image(log.after_image(), table)?;
        match log.sql_type() {
            Some("INSERT") if before == 0 && after > 0 => Ok(table),
            Some("UPDATE") if before == after => Ok(table),
            Some("DELETE") if after == 0 => Ok(table),
            _ => Err(Self::shape("JSONB SQL type or image direction")),
        }
    }

    fn physical_image(
        image: Option<&JacksonUndoRecords>,
        table: &str,
    ) -> Result<usize, UndoCodecError> {
        let image = image
            .filter(|image| image.table_name() == Some(table))
            .ok_or(Self::shape("JSONB physical image table identity"))?;
        for row in image.rows() {
            let mut names = HashSet::new();
            let mut primary = false;
            for field in row {
                TableMeta::validate_identifier(&field.name)
                    .map_err(|_| Self::shape("JSONB physical column identifier"))?;
                if !names.insert(field.name.to_ascii_lowercase()) {
                    return Err(Self::shape("JSONB duplicate physical column"));
                }
                if matches!(
                    field.value(),
                    JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::SerialArray(_))
                ) {
                    return Err(UndoCodecError::Unsupported {
                        kind: "JSONB physical field class",
                    });
                }
                if field.primary {
                    if matches!(
                        field.value(),
                        JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Null)
                    ) {
                        return Err(Self::shape("JSONB null physical primary field"));
                    }
                    primary = true;
                }
            }
            if !primary {
                return Err(Self::shape("JSONB missing physical primary field"));
            }
        }
        Ok(image.rows().len())
    }

    fn log(value: JsonbValue) -> Result<JacksonSqlUndoLog, UndoCodecError> {
        let mut fields = Self::object(value, None)?;
        let table = Self::text(fields.remove("tableName"))?;
        let kind = match fields.remove("sqlType") {
            Some(JsonbValue::Int(value)) => {
                let index = usize::try_from(value).map_err(|_| Self::shape("JSONB SQL type"))?;
                Some(
                    SQL_TYPE_LABELS
                        .get(index)
                        .ok_or(Self::shape("JSONB SQL type"))?
                        .to_string(),
                )
            }
            None | Some(JsonbValue::Null) => None,
            _ => return Err(Self::shape("JSONB SQL type")),
        };
        let before = Self::image(fields.remove("beforeImage"))?;
        let after = Self::image(fields.remove("afterImage"))?;
        Self::end(&fields)?;
        Ok(JacksonSqlUndoLog::new(kind, table, before, after))
    }

    fn image(value: Option<JsonbValue>) -> Result<Option<JacksonUndoRecords>, UndoCodecError> {
        let (empty, mut fields) = match value {
            None | Some(JsonbValue::Null) => return Ok(None),
            Some(JsonbValue::Object(None, fields)) => (false, fields),
            Some(JsonbValue::Object(Some(class), fields)) if class == EMPTY => (true, fields),
            _ => return Err(Self::shape("JSONB records class")),
        };
        let table = Self::text(fields.remove("tableName"))?;
        let rows = Self::list(fields.remove("rows"), 1024)?;
        Self::end(&fields)?;
        if empty {
            if !rows.is_empty() {
                return Err(Self::shape("JSONB empty records with rows"));
            }
            return Ok(Some(JacksonUndoRecords::empty(table)));
        }
        let rows = rows
            .into_iter()
            .map(Self::row)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(JacksonUndoRecords::new(table, rows)))
    }

    fn row(value: JsonbValue) -> Result<Vec<JacksonUndoField>, UndoCodecError> {
        let mut fields = Self::object(value, None)?;
        let row = Self::list(fields.remove("fields"), 128)?
            .into_iter()
            .map(Self::field)
            .collect();
        Self::end(&fields)?;
        row
    }

    fn field(value: JsonbValue) -> Result<JacksonUndoField, UndoCodecError> {
        let mut fields = Self::object(value, None)?;
        let name = Self::text(fields.remove("name"))?.ok_or(Self::shape("JSONB column name"))?;
        let primary = match fields.remove("keyType") {
            Some(JsonbValue::Int(1)) => true,
            None | Some(JsonbValue::Int(0)) => false,
            _ => return Err(Self::shape("JSONB key type")),
        };
        let ty = match fields.remove("type") {
            Some(JsonbValue::Int(ty)) => ty,
            None => 0,
            _ => return Err(Self::shape("JSONB JDBC type")),
        };
        let value = match fields.remove("value") {
            None | Some(JsonbValue::Null) => FastjsonUndoFieldValue::Null,
            Some(JsonbValue::Int(n)) => FastjsonUndoFieldValue::Integer(n),
            Some(JsonbValue::Long(n)) => FastjsonUndoFieldValue::Long(n),
            Some(JsonbValue::BigInteger(n)) => FastjsonUndoFieldValue::Unsigned(n),
            Some(JsonbValue::Decimal(value)) => FastjsonUndoFieldValue::Decimal(value),
            Some(JsonbValue::Binary(bytes)) => FastjsonUndoFieldValue::Binary(bytes),
            Some(JsonbValue::Text(text)) => FastjsonUndoFieldValue::Text(text),
            Some(JsonbValue::Bool(value)) => FastjsonUndoFieldValue::Boolean(value),
            Some(obj @ JsonbValue::Object(_, _)) => {
                FastjsonUndoFieldValue::SerialArray(Self::serial_array(obj)?)
            }
            _ => return Err(Self::shape("JSONB field value")),
        };
        Self::end(&fields)?;
        Ok(JacksonUndoField::new(
            name,
            ty,
            primary,
            JacksonUndoFieldValue::Common(value),
        ))
    }

    fn serial_array(value: JsonbValue) -> Result<SerialArray, UndoCodecError> {
        let mut fields = Self::object(value, Some(SERIAL_ARRAY))?;
        if !matches!(fields.remove("baseType"), Some(JsonbValue::Int(-5))) {
            return Err(Self::shape("JSONB SerialArray BIGINT base type"));
        }
        let name = Self::text(fields.remove("baseTypeName"))?
            .ok_or(Self::shape("JSONB SerialArray type name"))?;
        let elements = Self::list(fields.remove("elements"), 1024)?;
        let length = match fields.remove("len") {
            None => 0,
            Some(JsonbValue::Int(len)) => {
                usize::try_from(len).map_err(|_| Self::shape("JSONB SerialArray length"))?
            }
            _ => return Err(Self::shape("JSONB SerialArray length")),
        };
        Self::end(&fields)?;
        if elements.len() != length {
            return Err(Self::shape("JSONB SerialArray length"));
        }
        let values = elements
            .into_iter()
            .map(|element| match element {
                JsonbValue::Null => Ok(None),
                JsonbValue::Long(n) => Ok(Some(n)),
                _ => Err(Self::shape("JSONB SerialArray Long value")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SerialArray::bigint(name, values))
    }

    fn object(
        value: JsonbValue,
        class: Option<&str>,
    ) -> Result<BTreeMap<String, JsonbValue>, UndoCodecError> {
        match value {
            JsonbValue::Object(actual, fields) if actual.as_deref() == class => Ok(fields),
            _ => Err(Self::shape("JSONB object class")),
        }
    }

    fn list(value: Option<JsonbValue>, max: usize) -> Result<Vec<JsonbValue>, UndoCodecError> {
        match value {
            Some(JsonbValue::List(values)) if values.len() <= max => Ok(values),
            Some(JsonbValue::List(_)) => Err(UndoCodecError::Budget {
                limit: "JSONB Undo collection elements",
            }),
            _ => Err(Self::shape("JSONB collection")),
        }
    }

    fn text(value: Option<JsonbValue>) -> Result<Option<String>, UndoCodecError> {
        match value {
            None | Some(JsonbValue::Null) => Ok(None),
            Some(JsonbValue::Text(text)) => Ok(Some(text)),
            _ => Err(Self::shape("JSONB text")),
        }
    }

    fn end(fields: &BTreeMap<String, JsonbValue>) -> Result<(), UndoCodecError> {
        if fields.is_empty() {
            Ok(())
        } else {
            Err(Self::shape("unknown JSONB Undo member"))
        }
    }

    fn shape(reason: &'static str) -> UndoCodecError {
        UndoCodecError::Shape { reason }
    }
}
