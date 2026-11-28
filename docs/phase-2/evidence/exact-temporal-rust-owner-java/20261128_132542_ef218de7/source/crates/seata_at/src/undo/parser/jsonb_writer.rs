use super::{
    bounded_output::BoundedOutput,
    jackson_undo_log_parser::{BRANCH, EMPTY},
    serial_array::SERIAL_ARRAY,
    sql_type_labels::SQL_TYPE_LABELS,
    FastjsonUndoFieldValue, JacksonSqlUndoLog, JacksonUndoField, JacksonUndoFieldValue,
    JacksonUndoRecords, UndoCodecError,
};
use num_bigint::BigInt;
use std::{collections::HashMap, io::Write};

/// Fastjson2 JSONB 原 Undo 写入器，按原字段顺序登记类型和字段符号。
/// 对应 Java: com.alibaba.fastjson2.JSONWriterJSONB 的 Undo 值域。
pub(super) struct JsonbWriter<'a> {
    output: &'a mut BoundedOutput,
    symbols: HashMap<&'static str, i32>,
}

impl<'a> JsonbWriter<'a> {
    pub(super) fn branch(
        output: &'a mut BoundedOutput,
        xid: Option<&str>,
        branch_id: i64,
        logs: Option<&[JacksonSqlUndoLog]>,
    ) -> Result<(), UndoCodecError> {
        let mut writer = Self {
            output,
            symbols: HashMap::new(),
        };
        writer.begin_object(Some(BRANCH))?;
        if branch_id != 0 {
            writer.name("branchId")?;
            writer.long(branch_id)?;
        }
        writer.name("sqlUndoLogs")?;
        if let Some(logs) = logs {
            writer.array(logs.len())?;
            for log in logs {
                writer.log(log)?;
            }
        } else {
            writer.byte(0xaf)?;
        }
        writer.optional_string("xid", xid)?;
        writer.byte(0xa5)
    }

    fn log(&mut self, log: &JacksonSqlUndoLog) -> Result<(), UndoCodecError> {
        self.begin_object(None)?;
        self.name("afterImage")?;
        self.image(log.after_image())?;
        self.name("beforeImage")?;
        self.image(log.before_image())?;
        if let Some(kind) = log.sql_type() {
            let index = SQL_TYPE_LABELS
                .iter()
                .position(|label| *label == kind)
                .ok_or(UndoCodecError::Unsupported {
                    kind: "JSONB SQL type",
                })?;
            self.name("sqlType")?;
            self.int(Self::length(index)?)?;
        }
        self.optional_string("tableName", log.table_name())?;
        self.byte(0xa5)
    }

    fn image(&mut self, image: Option<&JacksonUndoRecords>) -> Result<(), UndoCodecError> {
        let Some(image) = image else {
            return self.byte(0xaf);
        };
        self.begin_object(image.is_empty_records().then_some(EMPTY))?;
        self.name("rows")?;
        self.array(image.rows().len())?;
        for row in image.rows() {
            self.begin_object(None)?;
            self.name("fields")?;
            self.array(row.len())?;
            for field in row {
                self.field(field)?;
            }
            self.byte(0xa5)?;
        }
        self.optional_string("tableName", image.table_name())?;
        self.byte(0xa5)
    }

    fn field(&mut self, field: &JacksonUndoField) -> Result<(), UndoCodecError> {
        self.begin_object(None)?;
        self.name("keyType")?;
        self.int(i32::from(field.primary))?;
        self.name("name")?;
        self.text(&field.name)?;
        if field.jdbc_type != 0 {
            self.name("type")?;
            self.int(field.jdbc_type)?;
        }
        self.name("value")?;
        match field.value() {
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Null) => self.byte(0xaf)?,
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Boolean(value)) => {
                self.byte(if *value { 0xb1 } else { 0xb0 })?
            }
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Integer(value)) => {
                self.int(*value)?
            }
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Long(value)) => {
                self.long(*value)?
            }
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Text(text)) => self.text(text)?,
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Unsigned(value)) => {
                self.big_integer(&BigInt::from(*value))?;
            }
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Decimal(value)) => {
                self.decimal(value)?;
            }
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Binary(bytes)) => {
                self.byte(0x92)?;
                self.symbol("[B")?;
                self.byte(0x91)?;
                self.int(Self::length(bytes.len())?)?;
                self.raw(bytes)?;
            }
            JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::SerialArray(array)) => {
                self.begin_object(Some(SERIAL_ARRAY))?;
                self.name("baseType")?;
                self.int(array.base_type())?;
                self.name("baseTypeName")?;
                self.text(array.base_type_name())?;
                self.name("elements")?;
                self.array(array.elements().len())?;
                for element in array.elements() {
                    match element {
                        Some(value) => self.long(*value)?,
                        None => self.byte(0xaf)?,
                    };
                }
                if !array.elements().is_empty() {
                    self.name("len")?;
                    self.int(Self::length(array.elements().len())?)?;
                }
                self.byte(0xa5)?;
            }
            _ => {
                return Err(UndoCodecError::Unsupported {
                    kind: "JSONB original field value",
                })
            }
        }
        self.byte(0xa5)
    }

    fn name(&mut self, name: &'static str) -> Result<(), UndoCodecError> {
        self.byte(0x7f)?;
        self.symbol(name)
    }

    fn symbol(&mut self, name: &'static str) -> Result<(), UndoCodecError> {
        if let Some(index) = self.symbols.get(name).copied() {
            return self.int(index);
        }
        let index = Self::length(self.symbols.len())?;
        self.symbols.insert(name, index);
        self.text(name)?;
        self.int(index)
    }

    fn begin_object(&mut self, class: Option<&'static str>) -> Result<(), UndoCodecError> {
        if let Some(class) = class {
            self.byte(0x92)?;
            self.symbol(class)?;
        }
        self.byte(0xa6)
    }

    fn optional_string(
        &mut self,
        name: &'static str,
        text: Option<&str>,
    ) -> Result<(), UndoCodecError> {
        if let Some(text) = text {
            self.name(name)?;
            self.text(text)?;
        }
        Ok(())
    }

    fn array(&mut self, count: usize) -> Result<(), UndoCodecError> {
        let count = Self::length(count)?;
        if count <= 15 {
            return self.byte(0x94 + count as u8);
        }
        self.byte(0xa4)?;
        self.int(count)
    }

    fn text(&mut self, text: &str) -> Result<(), UndoCodecError> {
        if text.is_ascii() {
            self.latin1_header(text.len())?;
            return self.raw(text.as_bytes());
        }
        if text.chars().all(|ch| u32::from(ch) <= 255) {
            self.latin1_header(text.chars().count())?;
            for ch in text.chars() {
                self.byte(u32::from(ch) as u8)?;
            }
            return Ok(());
        }

        // 固定 JDK 17 compact strings：采样 UTF-16 前 64 单元，
        // 对零字节较少或 UTF-8 更长的文本使用固定 oracle 的小端 UTF-16。
        let units = text.encode_utf16().count();
        let zero_bytes = text
            .encode_utf16()
            .take(64)
            .filter(|unit| unit.to_le_bytes().contains(&0))
            .count();
        let utf16_len = units.checked_mul(2).ok_or(UndoCodecError::Budget {
            limit: "JSONB length",
        })?;
        if zero_bytes == 0 || units.min(64) >= zero_bytes * 3 || text.len() > utf16_len {
            self.byte(124)?;
            self.int(Self::length(utf16_len)?)?;
            for unit in text.encode_utf16() {
                self.raw(&unit.to_le_bytes())?;
            }
            return Ok(());
        }
        self.byte(122)?;
        self.int(Self::length(text.len())?)?;
        self.raw(text.as_bytes())
    }

    fn latin1_header(&mut self, len: usize) -> Result<(), UndoCodecError> {
        if len <= 47 {
            return self.byte(73 + len as u8);
        }
        self.byte(121)?;
        self.int(Self::length(len)?)
    }

    fn int(&mut self, value: i32) -> Result<(), UndoCodecError> {
        match value {
            -16..=47 => self.byte(value as u8),
            -2048..=2047 => self.raw(&[(56 + (value >> 8)) as u8, value as u8]),
            -262144..=262143 => {
                self.raw(&[(68 + (value >> 16)) as u8, (value >> 8) as u8, value as u8])
            }
            _ => {
                self.byte(72)?;
                self.raw(&value.to_be_bytes())
            }
        }
    }

    fn long(&mut self, value: i64) -> Result<(), UndoCodecError> {
        match value {
            -8..=15 => self.byte((224 + value) as u8),
            -2048..=2047 => self.raw(&[(208 + (value >> 8)) as u8, value as u8]),
            -262144..=262143 => {
                self.raw(&[(196 + (value >> 16)) as u8, (value >> 8) as u8, value as u8])
            }
            _ => match i32::try_from(value) {
                Ok(value) => {
                    self.byte(191)?;
                    self.raw(&value.to_be_bytes())
                }
                Err(_) => {
                    self.byte(190)?;
                    self.raw(&value.to_be_bytes())
                }
            },
        }
    }

    fn big_integer(&mut self, value: &BigInt) -> Result<(), UndoCodecError> {
        if let Ok(compact) = i64::try_from(value) {
            self.byte(0xba)?;
            return self.long(compact);
        }
        let bytes = value.to_signed_bytes_be();
        self.byte(0xbb)?;
        self.int(Self::length(bytes.len())?)?;
        self.raw(&bytes)
    }

    fn decimal(&mut self, value: &seata_datasource::DecimalValue) -> Result<(), UndoCodecError> {
        // 保留 BigDecimal 的 scale，按原写入器选择最小的精确系数表示。
        let coefficient = value
            .as_str()
            .replace('.', "")
            .parse::<BigInt>()
            .map_err(|_| UndoCodecError::Shape {
                reason: "JSONB decimal coefficient",
            })?;
        let compact = i64::try_from(&coefficient).ok();
        if let (0, Some(compact)) = (value.scale(), compact) {
            self.byte(0xb8)?;
            return self.long(compact);
        }
        self.byte(0xb9)?;
        self.int(value.scale() as i32)?;
        match compact {
            Some(compact) => match i32::try_from(compact) {
                Ok(small) => self.int(small),
                Err(_) => self.long(compact),
            },
            None => self.big_integer(&coefficient),
        }
    }

    fn length(len: usize) -> Result<i32, UndoCodecError> {
        i32::try_from(len).map_err(|_| UndoCodecError::Budget {
            limit: "JSONB length",
        })
    }

    fn byte(&mut self, byte: u8) -> Result<(), UndoCodecError> {
        self.raw(&[byte])
    }

    fn raw(&mut self, data: &[u8]) -> Result<(), UndoCodecError> {
        self.output
            .write_all(data)
            .map_err(|_| UndoCodecError::Syntax)
    }
}
