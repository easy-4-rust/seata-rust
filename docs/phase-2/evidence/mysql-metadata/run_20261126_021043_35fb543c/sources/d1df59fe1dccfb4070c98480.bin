use super::{
    bounded_output::BoundedOutput, fory_class::ForyClass as Class, fory_reader::ForyReader,
    fory_writer::ForyWriter, sql_type_labels::SQL_TYPE_LABELS, FastjsonUndoFieldValue as Common,
    ForyUndoBranch, JacksonSqlUndoLog, JacksonUndoField, JacksonUndoFieldValue as Value,
    JacksonUndoRecords, UndoCodecError, UndoCodecLimits,
};

/// 固定 Fory 0.12.3 Java compatible 模式的有界原 Undo 编解码器。
/// 对应 Java: org.apache.seata.rm.datasource.undo.parser.ForyUndoLogParser。
#[derive(Debug, Default, Clone)]
pub struct ForyUndoLogParser {
    limits: UndoCodecLimits,
}

impl ForyUndoLogParser {
    /// 参数为检查过的字节与对象预算，返回独立编解码器。
    pub fn new(limits: UndoCodecLimits) -> Self {
        Self { limits }
    }

    /// 无参数；返回编码默认 Java 分支对象的 Fory 字节。
    pub fn default_content(&self) -> Result<Vec<u8>, UndoCodecError> {
        self.encode_branch(None, 0, None)
    }

    /// 参数为可空 XID、分支号及可空 SQL 日志；返回有界的原 Fory 字节。
    /// 对应 Java: ForyUndoLogParser#encode。多态等值文本、布尔值及 Java 装箱缓存范围内的整数共享引用。
    pub fn encode_branch(
        &self,
        xid: Option<&str>,
        branch_id: i64,
        logs: Option<&[JacksonSqlUndoLog]>,
    ) -> Result<Vec<u8>, UndoCodecError> {
        let mut offset = 0;
        let budget = self
            .limits
            .encoded
            .checked_sub(4)
            .ok_or(UndoCodecError::Budget {
                limit: "encoded output",
            })?;
        let data = BoundedOutput::generate(budget, |out| {
            let mut writer = ForyWriter::new(out);
            writer.fresh()?;
            writer.class(Class::Branch)?;
            writer.long(branch_id)?;
            writer.text_field(xid)?;
            if let Some(logs) = logs {
                writer.list(logs.len(), Class::SqlLog)?;
                for log in logs {
                    Self::write_log(&mut writer, log)?;
                }
            } else {
                writer.byte(0xfd)?;
            }
            offset = writer.finish()?;
            Ok(())
        })?;
        let mut bytes = Vec::with_capacity(4 + data.len());
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&data);
        self.decode_branch(&bytes)?;
        Ok(bytes)
    }

    /// 参数为完整 Fory 原 Undo 字节；返回受支持的原分支对象。
    /// 类描述必须匹配固定 Seata 类，解码不动态加载类或解析数据库元数据。
    pub fn decode_branch(&self, bytes: &[u8]) -> Result<ForyUndoBranch, UndoCodecError> {
        let mut reader = ForyReader::new(bytes, self.limits)?;
        reader.fresh(1)?;
        if reader.class()? != Class::Branch {
            return Err(Self::shape("Fory branch class"));
        }
        let branch_id = reader.numeric(ForyReader::long)?;
        let xid = reader.text_field(2)?;
        let logs = reader
            .list(2, 128, Class::SqlLog)?
            .map(|count| {
                (0..count)
                    .map(|_| Self::read_log(&mut reader))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        reader.finish()?;
        Ok(ForyUndoBranch {
            xid,
            branch_id,
            logs,
        })
    }

    fn write_log<'data>(
        out: &mut ForyWriter<'_, 'data>,
        log: &'data JacksonSqlUndoLog,
    ) -> Result<(), UndoCodecError> {
        out.fresh()?;
        out.text_field(log.table_name())?;
        if let Some(kind) = log.sql_type() {
            let ordinal = SQL_TYPE_LABELS
                .iter()
                .position(|label| *label == kind)
                .ok_or(Self::shape("Fory SQL type"))?;
            out.byte(0xff)?;
            out.var(ordinal as u32)?;
        } else {
            out.byte(0xfd)?;
        }
        Self::write_image(out, log.after_image())?;
        Self::write_image(out, log.before_image())
    }

    fn write_image<'data>(
        out: &mut ForyWriter<'_, 'data>,
        image: Option<&'data JacksonUndoRecords>,
    ) -> Result<(), UndoCodecError> {
        let Some(image) = image else {
            return out.byte(0xfd);
        };
        out.fresh()?;
        out.class(if image.is_empty_records() {
            Class::EmptyRecords
        } else {
            Class::Records
        })?;
        out.text_field(image.table_name())?;
        out.list(image.rows().len(), Class::Row)?;
        for row in image.rows() {
            out.fresh()?;
            out.list(row.len(), Class::Field)?;
            for field in row {
                out.fresh()?;
                out.int(field.jdbc_type)?;
                out.text_field(Some(&field.name))?;
                out.byte(0xff)?;
                out.var(u32::from(field.primary))?;
                Self::write_value(out, field.value())?;
            }
        }
        Ok(())
    }

    fn write_value<'data>(
        out: &mut ForyWriter<'_, 'data>,
        value: &'data Value,
    ) -> Result<(), UndoCodecError> {
        match value {
            Value::Common(Common::Null) => out.byte(0xfd),
            Value::Common(Common::Text(text)) => out.string_value(text),
            Value::Common(Common::Integer(value)) if (-128..=127).contains(value) => {
                out.number_value(0x24, i64::from(*value) as u64, |out| out.int(*value))
            }
            Value::Common(Common::Integer(value)) => {
                out.fresh()?;
                out.byte(0x24)?;
                out.int(*value)
            }
            Value::Common(Common::Long(value)) if (-128..=127).contains(value) => {
                out.number_value(0x28, *value as u64, |out| out.long(*value))
            }
            Value::Common(Common::Long(value)) => {
                out.fresh()?;
                out.byte(0x28)?;
                out.long(*value)
            }
            Value::Common(Common::Boolean(value)) => {
                out.number_value(0x1c, u64::from(*value), |out| out.byte(u8::from(*value)))
            }
            Value::Double(value) => {
                out.fresh()?;
                out.byte(0x2a)?;
                out.raw(&value.to_bits().to_le_bytes())
            }
            _ => Err(UndoCodecError::Unsupported {
                kind: "Fory Undo field value",
            }),
        }
    }

    fn read_log(input: &mut ForyReader<'_>) -> Result<JacksonSqlUndoLog, UndoCodecError> {
        input.fresh(3)?;
        let table = input.text_field(4)?;
        let kind = match input.byte()? {
            0xfd => None,
            0xff => {
                let ordinal = input.numeric(ForyReader::var)? as usize;
                Some(
                    SQL_TYPE_LABELS
                        .get(ordinal)
                        .ok_or(Self::shape("Fory SQL type"))?
                        .to_string(),
                )
            }
            _ => return Err(Self::shape("Fory SQL type marker")),
        };
        let after = Self::read_image(input)?;
        let before = Self::read_image(input)?;
        Ok(JacksonSqlUndoLog::new(kind, table, before, after))
    }

    fn read_image(
        input: &mut ForyReader<'_>,
    ) -> Result<Option<JacksonUndoRecords>, UndoCodecError> {
        input.node(4)?;
        match input.byte()? {
            0xfd => return Ok(None),
            0 => {
                input.define_reference();
            }
            _ => {
                return Err(UndoCodecError::Unsupported {
                    kind: "Fory image reference",
                })
            }
        }
        let empty = match input.class()? {
            Class::Records => false,
            Class::EmptyRecords => true,
            _ => return Err(Self::shape("Fory image class")),
        };
        let table = input.text_field(5)?;
        let count = input
            .list(5, 1024, Class::Row)?
            .ok_or(Self::shape("Fory rows list"))?;
        if empty && count > 0 {
            return Err(Self::shape("Fory empty records with rows"));
        }
        let rows = (0..count)
            .map(|_| {
                input.fresh(6)?;
                let count = input
                    .list(7, 128, Class::Field)?
                    .ok_or(Self::shape("Fory fields list"))?;
                (0..count).map(|_| Self::read_field(input)).collect()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if empty {
            Ok(Some(JacksonUndoRecords::empty(table)))
        } else {
            Ok(Some(JacksonUndoRecords::new(table, rows)))
        }
    }

    fn read_field(input: &mut ForyReader<'_>) -> Result<JacksonUndoField, UndoCodecError> {
        input.fresh(8)?;
        let jdbc = input.numeric(ForyReader::int)?;
        let name = input.text_field(9)?.ok_or(Self::shape("Fory field name"))?;
        input.expect(0xff)?;
        let primary = match input.numeric(ForyReader::var)? {
            0 => false,
            1 => true,
            _ => return Err(Self::shape("Fory field key type")),
        };
        let value = Self::read_value(input)?;
        Ok(JacksonUndoField::new(name, jdbc, primary, value))
    }

    fn read_value(input: &mut ForyReader<'_>) -> Result<Value, UndoCodecError> {
        input.node(9)?;
        let id = match input.byte()? {
            0xfd => return Ok(Value::Common(Common::Null)),
            0xfe => return input.read_scalar_reference(),
            0xff => None,
            0 => Some(input.define_reference()),
            _ => return Err(Self::shape("Fory field reference marker")),
        };
        let class = input.var()?;
        let start = input.remaining_len();
        let value = match class {
            0x24 => Value::Common(Common::Integer(input.numeric(ForyReader::int)?)),
            0x28 => Value::Common(Common::Long(input.numeric(ForyReader::long)?)),
            0x1c => {
                let value = match input.numeric(ForyReader::byte)? {
                    0 => false,
                    1 => true,
                    _ => return Err(Self::shape("Fory boolean")),
                };
                Value::Common(Common::Boolean(value))
            }
            0x2a => Value::Double(f64::from_bits(input.numeric(ForyReader::fixed64)?)),
            0x2c => Value::Common(Common::Text(input.text()?)),
            _ => {
                return Err(UndoCodecError::Unsupported {
                    kind: "Fory Undo field value class",
                })
            }
        };
        if let Some(id) = id {
            let len = match &value {
                Value::Common(Common::Text(value)) => value.len(),
                _ => start - input.remaining_len(),
            };
            input.define_scalar(id, &value, len);
        }
        Ok(value)
    }

    fn shape(reason: &'static str) -> UndoCodecError {
        UndoCodecError::Shape { reason }
    }
}
