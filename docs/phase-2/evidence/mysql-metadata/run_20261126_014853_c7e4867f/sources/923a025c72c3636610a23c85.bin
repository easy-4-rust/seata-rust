use super::{
    bounded_output::BoundedOutput, fastjson_undo_log_parser::BRANCH, fastjson_wire::FastjsonWire,
    serial_array::SERIAL_ARRAY, FastjsonSqlUndoLog, FastjsonUndoField, FastjsonUndoFieldValue,
    FastjsonUndoRecords, SerialArray, UndoCodecError, UndoCodecLimits,
};
use crate::SqlType;
use std::io::Write;

/// 按 Java Bean 的成员顺序及 Fastjson 原生字面量写出有界的 Undo 字节。
/// 对应 Java: FastjsonUndoLogParser#encode。
pub(crate) struct FastjsonBranchEncoder;

impl FastjsonBranchEncoder {
    pub(crate) fn encode(
        xid: Option<&str>,
        branch_id: i64,
        logs: Option<&[FastjsonSqlUndoLog]>,
        limits: UndoCodecLimits,
    ) -> Result<Vec<u8>, UndoCodecError> {
        let bytes = BoundedOutput::generate(limits.encoded, |out| {
            Self::write(
                out,
                format_args!(r#"{{"@type":"{BRANCH}","branchId":{branch_id}"#),
            )?;
            if let Some(logs) = logs {
                Self::raw(out, b",\"sqlUndoLogs\":[")?;
                for (index, log) in logs.iter().enumerate() {
                    if index != 0 {
                        Self::raw(out, b",")?;
                    }
                    Self::log(out, log)?;
                }
                Self::raw(out, b"]")?;
            }
            if let Some(xid) = xid {
                Self::raw(out, b",\"xid\":")?;
                Self::text(out, xid)?;
            }
            Self::raw(out, b"}")
        })?;
        // 同时应用接收侧结构及标量预算，确保编码产物可按相同预算解码。
        FastjsonWire::parse(&bytes, limits)?;
        Ok(bytes)
    }

    pub(crate) fn encode_array(
        array: &SerialArray,
        limits: UndoCodecLimits,
    ) -> Result<Vec<u8>, UndoCodecError> {
        let bytes = BoundedOutput::generate(limits.encoded, |out| Self::array(out, array, false))?;
        FastjsonWire::parse(&bytes, limits)?;
        Ok(bytes)
    }

    fn array(
        out: &mut BoundedOutput,
        array: &SerialArray,
        write_class_name: bool,
    ) -> Result<(), UndoCodecError> {
        Self::write(
            out,
            format_args!(
                "{{\"@type\":\"{SERIAL_ARRAY}\",\"baseType\":{},\"baseTypeName\":",
                array.base_type()
            ),
        )?;
        Self::text(out, array.base_type_name())?;
        Self::raw(out, b",\"elements\":[")?;
        for (index, element) in array.elements().iter().enumerate() {
            if index != 0 {
                Self::raw(out, b",")?;
            }
            match element {
                None => Self::raw(out, b"null")?,
                Some(value) if write_class_name => Self::long(out, *value)?,
                Some(value) => Self::write(out, format_args!("{value}"))?,
            }
        }
        Self::raw(out, b"]}")
    }

    fn long(out: &mut BoundedOutput, value: i64) -> Result<(), UndoCodecError> {
        Self::write(out, format_args!("{value}"))?;
        if i32::try_from(value).is_ok() {
            Self::raw(out, b"L")?;
        }
        Ok(())
    }

    fn log(out: &mut BoundedOutput, log: &FastjsonSqlUndoLog) -> Result<(), UndoCodecError> {
        Self::raw(out, b"{\"afterImage\":")?;
        Self::records(out, &log.after)?;
        Self::raw(out, b",\"beforeImage\":")?;
        Self::records(out, &log.before)?;
        Self::raw(out, b",\"sqlType\":")?;
        Self::text(
            out,
            match log.sql_type {
                SqlType::Insert => "INSERT",
                SqlType::Update => "UPDATE",
                SqlType::Delete => "DELETE",
            },
        )?;
        if let Some(table) = &log.table {
            Self::raw(out, b",\"tableName\":")?;
            Self::text(out, table)?;
        }
        Self::raw(out, b"}")
    }

    fn records(
        out: &mut BoundedOutput,
        records: &FastjsonUndoRecords,
    ) -> Result<(), UndoCodecError> {
        Self::raw(out, b"{\"rows\":[")?;
        for (row_index, row) in records.rows.iter().enumerate() {
            if row_index != 0 {
                Self::raw(out, b",")?;
            }
            Self::raw(out, b"{\"fields\":[")?;
            for (field_index, field) in row.iter().enumerate() {
                if field_index != 0 {
                    Self::raw(out, b",")?;
                }
                Self::field(out, field)?;
            }
            Self::raw(out, b"]}")?;
        }
        Self::raw(out, b"]")?;
        if let Some(table) = &records.table {
            Self::raw(out, b",\"tableName\":")?;
            Self::text(out, table)?;
        }
        Self::raw(out, b"}")
    }

    fn field(out: &mut BoundedOutput, field: &FastjsonUndoField) -> Result<(), UndoCodecError> {
        Self::raw(out, b"{\"keyType\":")?;
        Self::text(out, if field.primary { "PRIMARY_KEY" } else { "NULL" })?;
        Self::raw(out, b",\"name\":")?;
        Self::text(out, &field.name)?;
        Self::write(out, format_args!(",\"type\":{}", field.jdbc_type))?;
        if !matches!(&field.value, FastjsonUndoFieldValue::Null) {
            Self::raw(out, b",\"value\":")?;
            Self::value(out, &field.value)?;
        }
        Self::raw(out, b"}")
    }

    fn value(
        out: &mut BoundedOutput,
        value: &FastjsonUndoFieldValue,
    ) -> Result<(), UndoCodecError> {
        match value {
            FastjsonUndoFieldValue::Null => Self::raw(out, b"null"),
            FastjsonUndoFieldValue::Boolean(value) => Self::write(out, format_args!("{value}")),
            FastjsonUndoFieldValue::Integer(value) => Self::write(out, format_args!("{value}")),
            FastjsonUndoFieldValue::Long(value) => Self::long(out, *value),
            FastjsonUndoFieldValue::SerialArray(value) => Self::array(out, value, true),
            FastjsonUndoFieldValue::Unsigned(value) => Self::write(out, format_args!("{value}")),
            FastjsonUndoFieldValue::Decimal(value) => {
                Self::raw(out, value.java_big_decimal_text().as_bytes())?;
                if value.scale() == 0 {
                    Self::raw(out, b".")?;
                }
                Ok(())
            }
            FastjsonUndoFieldValue::Text(value) => Self::text(out, value),
            FastjsonUndoFieldValue::Binary(bytes) => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                Self::raw(out, b"x'")?;
                for &byte in bytes {
                    Self::raw(
                        out,
                        &[HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 15)]],
                    )?;
                }
                Self::raw(out, b"'")
            }
        }
    }

    fn text(out: &mut BoundedOutput, text: &str) -> Result<(), UndoCodecError> {
        Self::raw(out, b"\"")?;
        let mut start = 0;
        for (index, character) in text.char_indices() {
            let short = match character {
                '\u{8}' => Some(b"\\b"),
                '\t' => Some(b"\\t"),
                '\n' => Some(b"\\n"),
                '\u{c}' => Some(b"\\f"),
                '\r' => Some(b"\\r"),
                '"' => Some(b"\\\""),
                '\\' => Some(b"\\\\"),
                _ => None,
            };
            let unicode = matches!(character, '\0'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{2028}' | '\u{2029}');
            if short.is_none() && !unicode {
                continue;
            }
            // Fastjson 默认使用大写十六进制，并转义 DEL、C1 和 Unicode 行分隔符。
            Self::raw(out, &text.as_bytes()[start..index])?;
            if let Some(short) = short {
                Self::raw(out, short)?;
            } else {
                Self::write(out, format_args!("\\u{:04X}", u32::from(character)))?;
            }
            start = index + character.len_utf8();
        }
        Self::raw(out, &text.as_bytes()[start..])?;
        Self::raw(out, b"\"")
    }

    fn raw(out: &mut BoundedOutput, bytes: &[u8]) -> Result<(), UndoCodecError> {
        out.write_all(bytes).map_err(|_| UndoCodecError::Syntax)
    }

    fn write(
        out: &mut BoundedOutput,
        format: std::fmt::Arguments<'_>,
    ) -> Result<(), UndoCodecError> {
        out.write_fmt(format).map_err(|_| UndoCodecError::Syntax)
    }
}
