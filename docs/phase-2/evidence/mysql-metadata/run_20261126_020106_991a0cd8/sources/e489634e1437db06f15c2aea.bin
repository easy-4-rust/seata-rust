use super::{
    bounded_output::BoundedOutput,
    dmdb_timestamp::{DmdbTimestamp, DMDB_TIMESTAMP},
    jackson_undo_log_parser::{BRANCH, EMPTY, FIELD, RECORDS, ROW, SQL},
    restored_format::RestoredFormat,
    serial_array::SERIAL_ARRAY,
    FastjsonSqlUndoLog, FastjsonUndoField, FastjsonUndoFieldValue, FastjsonUndoRecords,
    JacksonSqlUndoLog, JacksonUndoField, JacksonUndoFieldValue, JacksonUndoRecords, SerialArray,
    UndoCodecError, UndoCodecLimits,
};
use crate::SqlType;
use base64::{engine::general_purpose::STANDARD, write::EncoderWriter, Engine};
use serde::Serialize;
use serde_json::{
    ser::{CharEscape, CompactFormatter, Formatter, Serializer},
    Value,
};
use std::io::{self, Write};

/// Jackson 普通 Bean、日志及标量的格式适配，复用原对象载体。
/// 对应 Java: JacksonUndoLogParser 的 BranchUndoLog、SQLUndoLog、TableRecords 与 Field。
pub(super) struct JacksonOriginalFormat;

impl JacksonOriginalFormat {
    pub(super) fn encode<L>(
        xid: Option<&str>,
        branch_id: i64,
        logs: Option<&[L]>,
        limits: UndoCodecLimits,
        write: impl FnMut(&mut BoundedOutput, &L) -> Result<(), UndoCodecError>,
    ) -> Result<Vec<u8>, UndoCodecError> {
        BoundedOutput::generate(limits.encoded, |out| {
            Self::tag(out, BRANCH)?;
            Self::raw(out, b",\"xid\":")?;
            Self::json(out, &xid)?;
            Self::raw(out, b",\"branchId\":")?;
            Self::json(out, &branch_id)?;
            Self::raw(out, b",\"sqlUndoLogs\":")?;
            match logs {
                Some(logs) => Self::list(out, logs, write)?,
                None => Self::raw(out, b"null")?,
            }
            Self::raw(out, b"}")
        })
    }

    pub(super) fn write_log(
        out: &mut BoundedOutput,
        log: &FastjsonSqlUndoLog,
    ) -> Result<(), UndoCodecError> {
        let sql_type = match log.sql_type {
            SqlType::Insert => "INSERT",
            SqlType::Update => "UPDATE",
            SqlType::Delete => "DELETE",
        };
        Self::write_log_prefix(out, Some(sql_type), log.table.as_deref())?;
        Self::write_image(
            out,
            RECORDS,
            log.before.table.as_deref(),
            &log.before.rows,
            Self::write_field,
        )?;
        Self::raw(out, b",\"afterImage\":")?;
        Self::write_image(
            out,
            RECORDS,
            log.after.table.as_deref(),
            &log.after.rows,
            Self::write_field,
        )?;
        Self::raw(out, b"}")
    }

    pub(super) fn write_typed_log(
        out: &mut BoundedOutput,
        log: &JacksonSqlUndoLog,
    ) -> Result<(), UndoCodecError> {
        Self::write_log_prefix(out, log.sql_type(), log.table_name())?;
        Self::write_typed_image(out, log.before_image())?;
        Self::raw(out, b",\"afterImage\":")?;
        Self::write_typed_image(out, log.after_image())?;
        Self::raw(out, b"}")
    }

    fn write_log_prefix(
        out: &mut BoundedOutput,
        sql_type: Option<&str>,
        table: Option<&str>,
    ) -> Result<(), UndoCodecError> {
        Self::tag(out, SQL)?;
        Self::raw(out, b",\"sqlType\":")?;
        Self::json(out, &sql_type)?;
        Self::raw(out, b",\"tableName\":")?;
        Self::json(out, &table)?;
        Self::raw(out, b",\"beforeImage\":")
    }

    fn write_typed_image(
        out: &mut BoundedOutput,
        image: Option<&JacksonUndoRecords>,
    ) -> Result<(), UndoCodecError> {
        let Some(image) = image else {
            return Self::raw(out, b"null");
        };
        let class = if image.is_empty_records() {
            EMPTY
        } else {
            RECORDS
        };
        Self::write_image(
            out,
            class,
            image.table_name(),
            image.rows(),
            Self::write_typed_field,
        )
    }

    fn write_image<F>(
        out: &mut BoundedOutput,
        class: &str,
        table: Option<&str>,
        rows: &[Vec<F>],
        write: fn(&mut BoundedOutput, &F) -> Result<(), UndoCodecError>,
    ) -> Result<(), UndoCodecError> {
        Self::tag(out, class)?;
        Self::raw(out, b",\"tableName\":")?;
        Self::json(out, &table)?;
        Self::raw(out, b",\"rows\":")?;
        Self::list(out, rows, |out, row| {
            Self::tag(out, ROW)?;
            Self::raw(out, b",\"fields\":")?;
            Self::list(out, row, write)?;
            Self::raw(out, b"}")
        })?;
        Self::raw(out, b"}")
    }

    pub(super) fn write_field(
        out: &mut BoundedOutput,
        field: &FastjsonUndoField,
    ) -> Result<(), UndoCodecError> {
        Self::write_field_prefix(out, &field.name, field.jdbc_type, field.primary)?;
        Self::write_common_value(out, &field.value)?;
        Self::raw(out, b"}")
    }

    fn write_field_prefix(
        out: &mut BoundedOutput,
        name: &str,
        jdbc_type: i32,
        primary: bool,
    ) -> Result<(), UndoCodecError> {
        Self::tag(out, FIELD)?;
        Self::raw(out, b",\"name\":")?;
        Self::json(out, &name)?;
        Self::raw(
            out,
            if primary {
                b",\"keyType\":\"PRIMARY_KEY\",\"type\":"
            } else {
                b",\"keyType\":\"NULL\",\"type\":"
            },
        )?;
        Self::json(out, &jdbc_type)?;
        Self::raw(out, b",\"value\":")
    }

    fn write_common_value(
        out: &mut BoundedOutput,
        value: &FastjsonUndoFieldValue,
    ) -> Result<(), UndoCodecError> {
        match value {
            FastjsonUndoFieldValue::Null => Self::raw(out, b"null")?,
            FastjsonUndoFieldValue::Boolean(value) => Self::json(out, value)?,
            FastjsonUndoFieldValue::Integer(value) => Self::json(out, value)?,
            FastjsonUndoFieldValue::Text(value) => Self::json(out, value)?,
            FastjsonUndoFieldValue::Long(value) => Self::json(out, &("java.lang.Long", value))?,
            FastjsonUndoFieldValue::Unsigned(value) => {
                Self::json(out, &("java.math.BigInteger", value))?
            }
            FastjsonUndoFieldValue::Decimal(value) => {
                Self::raw(out, b"[\"java.math.BigDecimal\",")?;
                Self::raw(out, value.java_big_decimal_text().as_bytes())?;
                Self::raw(out, b"]")?;
            }
            FastjsonUndoFieldValue::Binary(value) => Self::write_binary(out, "[B", value)?,
            FastjsonUndoFieldValue::SerialArray(value) => {
                Self::tag(out, SERIAL_ARRAY)?;
                Self::raw(out, b",\"baseType\":")?;
                Self::json(out, &value.base_type())?;
                Self::raw(out, b",\"baseTypeName\":")?;
                Self::json(out, &value.base_type_name())?;
                Self::raw(out, b",\"elements\":")?;
                // 原序列化器逐个写入 Long 对象，元素以普通数字或 null 表示。
                Self::json(out, &value.elements())?;
                Self::raw(out, b"}")?;
            }
        }
        Ok(())
    }

    fn write_binary(
        out: &mut BoundedOutput,
        class: &str,
        value: &[u8],
    ) -> Result<(), UndoCodecError> {
        Self::raw(out, b"[")?;
        Self::json(out, &class)?;
        Self::raw(out, b",\"")?;
        {
            let mut encoded = EncoderWriter::new(&mut *out, &STANDARD);
            encoded
                .write_all(value)
                .map_err(|_| UndoCodecError::Syntax)?;
            encoded.finish().map_err(|_| UndoCodecError::Syntax)?;
        }
        Self::raw(out, b"\"]")
    }

    pub(super) fn write_typed_field(
        out: &mut BoundedOutput,
        field: &JacksonUndoField,
    ) -> Result<(), UndoCodecError> {
        use JacksonUndoFieldValue as V;
        Self::write_field_prefix(out, &field.name, field.jdbc_type, field.primary)?;
        let class = field.value.java_class();
        match &field.value {
            V::Common(value) => Self::write_common_value(out, value)?,
            V::Double(value) if value.is_finite() => Self::json(out, value)?,
            V::Double(_) => return Err(Self::unsupported()),
            V::Timestamp {
                epoch_seconds,
                nanos,
            }
            | V::LocalDateTimeUtc {
                epoch_seconds,
                nanos,
            } => {
                Self::write_timestamp(out, class, *epoch_seconds, *nanos)?;
            }
            V::DmdbTimestamp(value) => {
                Self::write_timestamp(out, class, value.epoch_seconds, value.nanos)?;
            }
            V::OffsetDateTime(value) => {
                Self::raw(out, b"[")?;
                Self::json(out, &class)?;
                Self::raw(out, b",")?;
                Self::raw(out, value.encode_jackson_timestamp().as_bytes())?;
                Self::raw(out, b"]")?;
            }
            V::SerialBlob(bytes) if !bytes.is_empty() => Self::write_binary(out, class, bytes)?,
            // 固定 JDK 的 SerialBlob#getBytes(1, 0) 失败，原写入只留下类标签。
            V::SerialBlob(_) => return Err(Self::unsupported()),
            V::SerialClob(text) => Self::json(out, &(class, text))?,
        }
        Self::raw(out, b"}")
    }

    fn write_timestamp(
        out: &mut BoundedOutput,
        class: &str,
        epoch_seconds: i64,
        nanos: u32,
    ) -> Result<(), UndoCodecError> {
        if nanos >= 1_000_000_000 {
            return Err(Self::unsupported());
        }
        // 先以 i128 算完整毫秒，保留 i64::MIN 边界上的合法秒内值。
        let millis =
            i64::try_from(i128::from(epoch_seconds) * 1000 + i128::from(nanos / 1_000_000))
                .map_err(|_| Self::unsupported())?;
        if nanos % 1_000_000 == 0 {
            Self::json(out, &(class, millis))
        } else {
            Self::json(out, &(class, (millis, nanos)))
        }
    }

    fn list<T>(
        out: &mut BoundedOutput,
        values: &[T],
        mut write: impl FnMut(&mut BoundedOutput, &T) -> Result<(), UndoCodecError>,
    ) -> Result<(), UndoCodecError> {
        Self::raw(out, b"[\"java.util.ArrayList\",[")?;
        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                Self::raw(out, b",")?;
            }
            write(out, value)?;
        }
        Self::raw(out, b"]]")
    }

    fn tag(out: &mut BoundedOutput, name: &str) -> Result<(), UndoCodecError> {
        Self::raw(out, b"{\"@class\":")?;
        Self::json(out, &name)
    }

    fn json(out: &mut BoundedOutput, value: &impl Serialize) -> Result<(), UndoCodecError> {
        value
            .serialize(&mut Serializer::with_formatter(out, JacksonUtf8Formatter))
            .map_err(|_| UndoCodecError::Syntax)
    }

    fn raw(out: &mut BoundedOutput, bytes: &[u8]) -> Result<(), UndoCodecError> {
        out.write_all(bytes).map_err(|_| UndoCodecError::Syntax)
    }

    pub(super) fn logs(logs: &[Value]) -> Result<Vec<FastjsonSqlUndoLog>, UndoCodecError> {
        logs.iter()
            .map(|log| {
                let sql_type = match log.get("sqlType").and_then(Value::as_str) {
                    Some("INSERT") => SqlType::Insert,
                    Some("UPDATE") => SqlType::Update,
                    Some("DELETE") => SqlType::Delete,
                    _ => return Err(Self::unsupported()),
                };
                let table = log
                    .get("tableName")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let before = Self::image(log.get("beforeImage").ok_or_else(Self::unsupported)?)?;
                let after = Self::image(log.get("afterImage").ok_or_else(Self::unsupported)?)?;
                Ok(FastjsonSqlUndoLog::new(sql_type, table, before, after))
            })
            .collect()
    }

    fn image(image: &Value) -> Result<FastjsonUndoRecords, UndoCodecError> {
        if image.get("@class").and_then(Value::as_str) != Some(RECORDS) {
            return Err(Self::unsupported());
        }
        let table = image
            .get("tableName")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let rows = Self::rows(image, Self::field)?;
        Ok(FastjsonUndoRecords::new(table, rows))
    }

    pub(super) fn typed_logs(logs: &[Value]) -> Result<Vec<JacksonSqlUndoLog>, UndoCodecError> {
        logs.iter()
            .map(|log| {
                if log.is_null() {
                    return Err(Self::unsupported());
                }
                let sql_type = log
                    .get("sqlType")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let table = log
                    .get("tableName")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let before = Self::typed_image(log.get("beforeImage"))?;
                let after = Self::typed_image(log.get("afterImage"))?;
                Ok(JacksonSqlUndoLog::new(sql_type, table, before, after))
            })
            .collect()
    }

    fn typed_image(image: Option<&Value>) -> Result<Option<JacksonUndoRecords>, UndoCodecError> {
        let Some(image) = image.filter(|image| !image.is_null()) else {
            return Ok(None);
        };
        let table = image
            .get("tableName")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let rows = Self::rows(image, Self::typed_field)?;
        match image.get("@class").and_then(Value::as_str) {
            Some(RECORDS) => Ok(Some(JacksonUndoRecords::new(table, rows))),
            Some(EMPTY) if rows.is_empty() => Ok(Some(JacksonUndoRecords::empty(table))),
            _ => Err(Self::unsupported()),
        }
    }

    fn rows<F>(
        image: &Value,
        decode: fn(&Value) -> Result<F, UndoCodecError>,
    ) -> Result<Vec<Vec<F>>, UndoCodecError> {
        Self::contents(image.get("rows"))?
            .iter()
            .map(|row| {
                Self::contents(row.get("fields"))?
                    .iter()
                    .map(decode)
                    .collect()
            })
            .collect()
    }

    fn field_metadata(field: &Value) -> Result<(&str, i32, bool), UndoCodecError> {
        let name = field
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(Self::unsupported)?;
        let jdbc = field
            .get("type")
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok())
            .ok_or_else(Self::unsupported)?;
        let primary = match field.get("keyType").and_then(Value::as_str) {
            Some("PRIMARY_KEY") => true,
            Some("NULL") => false,
            _ => return Err(Self::unsupported()),
        };
        Ok((name, jdbc, primary))
    }

    pub(super) fn field(field: &Value) -> Result<FastjsonUndoField, UndoCodecError> {
        let (name, jdbc, primary) = Self::field_metadata(field)?;
        let raw = field.get("value").unwrap_or(&Value::Null);
        let value = Self::common_value(raw)?;
        Ok(FastjsonUndoField::new(name.into(), jdbc, primary, value))
    }

    fn common_value(raw: &Value) -> Result<FastjsonUndoFieldValue, UndoCodecError> {
        let value = match raw {
            Value::Object(array)
                if array.get("@class").and_then(Value::as_str) == Some(SERIAL_ARRAY) =>
            {
                if array.get("baseType").and_then(Value::as_i64) != Some(-5) {
                    return Err(Self::unsupported());
                }
                let name = array
                    .get("baseTypeName")
                    .and_then(Value::as_str)
                    .ok_or_else(Self::unsupported)?;
                let elements = array
                    .get("elements")
                    .and_then(Value::as_array)
                    .ok_or_else(Self::unsupported)?;
                let elements = elements
                    .iter()
                    .map(|value| {
                        if value.is_null() {
                            Ok(None)
                        } else {
                            value.as_i64().map(Some).ok_or_else(Self::unsupported)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                FastjsonUndoFieldValue::SerialArray(SerialArray::bigint(name.into(), elements))
            }
            Value::Array(pair) => {
                let class = pair
                    .first()
                    .and_then(Value::as_str)
                    .ok_or_else(Self::unsupported)?;
                let value = pair.get(1).ok_or_else(Self::unsupported)?;
                match class {
                    "java.lang.Boolean" => FastjsonUndoFieldValue::Boolean(
                        value.as_bool().ok_or_else(Self::unsupported)?,
                    ),
                    "java.lang.String" => FastjsonUndoFieldValue::Text(
                        value.as_str().ok_or_else(Self::unsupported)?.to_owned(),
                    ),
                    "java.lang.Integer" => FastjsonUndoFieldValue::Integer(
                        value
                            .as_i64()
                            .and_then(|n| i32::try_from(n).ok())
                            .ok_or_else(Self::unsupported)?,
                    ),
                    "java.lang.Long" => {
                        FastjsonUndoFieldValue::Long(value.as_i64().ok_or_else(Self::unsupported)?)
                    }
                    "java.math.BigInteger" => FastjsonUndoFieldValue::Unsigned(
                        value.as_u64().ok_or_else(Self::unsupported)?,
                    ),
                    "java.math.BigDecimal" => {
                        FastjsonUndoFieldValue::Decimal(RestoredFormat::decimal(
                            &value.as_number().ok_or_else(Self::unsupported)?.to_string(),
                        )?)
                    }
                    "[B" => FastjsonUndoFieldValue::Binary(
                        STANDARD
                            .decode(value.as_str().ok_or_else(Self::unsupported)?)
                            .map_err(|_| Self::unsupported())?,
                    ),
                    _ => return Err(Self::unsupported()),
                }
            }
            Value::Number(value) if value.to_string().contains(['.', 'e', 'E']) => {
                return Err(Self::unsupported());
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                FastjsonUndoFieldValue::from_wire(raw)?
            }
            _ => return Err(Self::unsupported()),
        };
        Ok(value)
    }

    pub(super) fn typed_field(field: &Value) -> Result<JacksonUndoField, UndoCodecError> {
        use JacksonUndoFieldValue as V;
        let (name, jdbc, primary) = Self::field_metadata(field)?;
        let raw = field.get("value").unwrap_or(&Value::Null);
        let value = match raw {
            Value::Array(pair) if pair.len() == 2 => match pair[0].as_str() {
                Some("java.sql.Timestamp" | "java.time.LocalDateTime") => {
                    let local = pair[0].as_str() == Some("java.time.LocalDateTime");
                    let (epoch_seconds, nanos) = Self::timestamp_parts(&pair[1], local)?;
                    if !local {
                        V::Timestamp {
                            epoch_seconds,
                            nanos,
                        }
                    } else {
                        V::LocalDateTimeUtc {
                            epoch_seconds,
                            nanos,
                        }
                    }
                }
                Some(DMDB_TIMESTAMP) => {
                    let (epoch_seconds, nanos) = Self::timestamp_parts(&pair[1], true)?;
                    V::DmdbTimestamp(DmdbTimestamp::from_zoned_utc(epoch_seconds, nanos)?)
                }
                Some(super::offset_date_time::OFFSET_DATE_TIME) => {
                    V::OffsetDateTime(super::OffsetDateTime::decode_number(
                        pair[1].as_number().ok_or_else(Self::unsupported)?,
                    )?)
                }
                Some("javax.sql.rowset.serial.SerialBlob") => V::SerialBlob(
                    STANDARD
                        .decode(pair[1].as_str().ok_or_else(Self::unsupported)?)
                        .map_err(|_| Self::unsupported())?,
                ),
                Some("javax.sql.rowset.serial.SerialClob") => {
                    V::SerialClob(pair[1].as_str().ok_or_else(Self::unsupported)?.into())
                }
                _ => V::Common(Self::common_value(raw)?),
            },
            Value::Number(number) if number.to_string().contains(['.', 'e', 'E']) => V::Double(
                number
                    .as_f64()
                    .filter(|number| number.is_finite())
                    .ok_or_else(Self::unsupported)?,
            ),
            _ => V::Common(Self::common_value(raw)?),
        };
        Ok(JacksonUndoField::new(name.into(), jdbc, primary, value))
    }

    fn timestamp_parts(value: &Value, local: bool) -> Result<(i64, u32), UndoCodecError> {
        let (millis, nanos) = if let Some(pair) = value.as_array() {
            let millis = pair
                .first()
                .and_then(Value::as_i64)
                .ok_or_else(Self::unsupported)?;
            let nanos = pair
                .get(1)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(Self::unsupported)?;
            (millis, Some(nanos))
        } else {
            (value.as_i64().ok_or_else(Self::unsupported)?, None)
        };
        let from_millis =
            u32::try_from(millis.rem_euclid(1000)).map_err(|_| Self::unsupported())? * 1_000_000;
        // Timestamp#setNanos 覆盖秒内值；LocalDateTime 只把亚毫秒位加到 Instant。
        let nanos = match nanos {
            Some(nanos) if local => from_millis + nanos % 1_000_000,
            Some(nanos) => nanos,
            None => from_millis,
        };
        Ok((millis.div_euclid(1000), nanos))
    }

    fn contents(value: Option<&Value>) -> Result<&[Value], UndoCodecError> {
        value
            .and_then(Value::as_array)
            .and_then(|pair| pair.get(1))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .ok_or_else(Self::unsupported)
    }

    fn unsupported() -> UndoCodecError {
        UndoCodecError::Unsupported {
            kind: "original Jackson object projection",
        }
    }
}

/// 固定 Jackson UTF8JsonGenerator 的文本转义规则。
struct JacksonUtf8Formatter;

impl Formatter for JacksonUtf8Formatter {
    fn write_string_fragment<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut start = 0;
        for (index, ch) in fragment
            .char_indices()
            .filter(|(_, ch)| ch.len_utf16() == 2)
        {
            writer.write_all(&fragment.as_bytes()[start..index])?;
            // Java 的 UTF-8 生成器将补充字符写为一对 UTF-16 转义。
            for unit in ch.encode_utf16(&mut [0; 2]) {
                write!(writer, "\\u{unit:04X}")?;
            }
            start = index + ch.len_utf8();
        }
        writer.write_all(&fragment.as_bytes()[start..])
    }

    fn write_char_escape<W: Write + ?Sized>(
        &mut self,
        writer: &mut W,
        escape: CharEscape,
    ) -> io::Result<()> {
        if let CharEscape::AsciiControl(code) = escape {
            return write!(writer, "\\u{code:04X}");
        }
        CompactFormatter.write_char_escape(writer, escape)
    }
}
