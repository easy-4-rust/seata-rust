use super::{
    bounded_output::BoundedOutput, kryo_class::KryoClass as Class, kryo_reader::KryoReader,
    kryo_writer::KryoWriter, sql_type_labels::SQL_TYPE_LABELS, FastjsonUndoFieldValue as Common,
    JacksonSqlUndoLog, JacksonUndoField, JacksonUndoFieldValue as Value, JacksonUndoRecords,
    KryoUndoBranch, UndoCodecError, UndoCodecLimits,
};

/// 有界的固定 Kryo 5.4 原 Undo 对象编解码。
/// 对应 Java: org.apache.seata.rm.datasource.undo.parser.KryoUndoLogParser。
#[derive(Debug, Default, Clone)]
pub struct KryoUndoLogParser {
    limits: UndoCodecLimits,
}

impl KryoUndoLogParser {
    /// 参数为已校验的编解码预算，返回独立原对象编解码器。
    pub fn new(limits: UndoCodecLimits) -> Self {
        Self { limits }
    }

    /// 无参数；返回固定 Kryo 工厂编码默认原分支的字节。
    pub fn default_content(&self) -> Result<Vec<u8>, UndoCodecError> {
        self.encode_branch(None, 0, None)
    }

    /// 参数为原可空 XID、分支号和日志列表；返回有界 Kryo 字节。
    /// 相同字符串值共享引用，表行对象按出现顺序独立编码。
    pub fn encode_branch(
        &self,
        xid: Option<&str>,
        branch_id: i64,
        logs: Option<&[JacksonSqlUndoLog]>,
    ) -> Result<Vec<u8>, UndoCodecError> {
        let bytes = BoundedOutput::generate(self.limits.encoded, |out| {
            let mut out = KryoWriter::new(out);
            out.class(Class::Branch)?;
            out.fresh()?;
            out.i64(branch_id)?;
            if let Some(logs) = logs {
                out.list_header(logs.len(), Class::Sql)?;
                for log in logs {
                    Self::write_log(&mut out, log)?;
                }
            } else {
                out.byte(0)?;
            }
            out.string_ref(xid)
        })?;
        self.decode_branch(&bytes)?;
        Ok(bytes)
    }

    /// 参数为完整固定 Kryo 原 Undo 字节，返回有界原分支及支持的字段值。
    /// 对应 Java: KryoUndoLogParser#decode；不加载动态类型。
    pub fn decode_branch(&self, bytes: &[u8]) -> Result<KryoUndoBranch, UndoCodecError> {
        let mut input = KryoReader::new(bytes, self.limits)?;
        if input.class()? != Some(Class::Branch) {
            return Err(Self::shape("Kryo branch class"));
        }
        input.fresh(1)?;
        let branch_id = input.numeric(KryoReader::i64)?;
        let logs = input
            .list(2, 128, Class::Sql)?
            .map(|count| {
                (0..count)
                    .map(|_| Self::read_log(&mut input))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let xid = input.string_ref(2)?;
        input.finish()?;
        Ok(KryoUndoBranch {
            xid,
            branch_id,
            logs,
        })
    }

    fn write_log<'a>(
        out: &mut KryoWriter<'_, 'a>,
        log: &'a JacksonSqlUndoLog,
    ) -> Result<(), UndoCodecError> {
        out.fresh()?;
        Self::write_image(out, log.after_image())?;
        Self::write_image(out, log.before_image())?;
        let ordinal = log
            .sql_type()
            .map(|kind| {
                SQL_TYPE_LABELS
                    .iter()
                    .position(|label| *label == kind)
                    .map(|index| index as u32 + 1)
                    .ok_or(Self::shape("Kryo SQL type"))
            })
            .transpose()?
            .unwrap_or_default();
        if ordinal != 0 {
            out.byte(1)?;
        }
        out.u32(ordinal)?;
        out.string_ref(log.table_name())
    }

    fn write_image<'a>(
        out: &mut KryoWriter<'_, 'a>,
        image: Option<&'a JacksonUndoRecords>,
    ) -> Result<(), UndoCodecError> {
        let Some(image) = image else {
            return out.byte(0);
        };
        let class = if image.is_empty_records() {
            Class::EmptyRecords
        } else {
            Class::Records
        };
        out.class(class)?;
        out.fresh()?;
        out.list_header(image.rows().len(), Class::Row)?;
        for row in image.rows() {
            out.fresh()?;
            out.list_header(row.len(), Class::Field)?;
            for field in row {
                out.fresh()?;
                // Kryo 的可空枚举先写非空标记，再写加一后的 ordinal。
                out.byte(1)?;
                out.byte(if field.primary { 2 } else { 1 })?;
                out.string_ref(Some(&field.name))?;
                out.i32(field.jdbc_type)?;
                Self::write_value(out, field.value())?;
            }
        }
        out.string_ref(image.table_name())
    }

    fn write_value<'a>(
        out: &mut KryoWriter<'_, 'a>,
        value: &'a Value,
    ) -> Result<(), UndoCodecError> {
        match value {
            Value::Common(Common::Null) => out.byte(0),
            Value::Common(Common::Integer(value)) => {
                out.class(Class::Integer)?;
                out.i32(*value)
            }
            Value::Common(Common::Long(value)) => {
                out.class(Class::Long)?;
                out.i64(*value)
            }
            Value::Common(Common::Boolean(value)) => {
                out.class(Class::Boolean)?;
                out.byte(u8::from(*value))
            }
            Value::Common(Common::Text(value)) => {
                out.class(Class::String)?;
                out.string_ref(Some(value))
            }
            Value::Double(value) => {
                out.class(Class::Double)?;
                let bits = if value.is_nan() {
                    0x7ff8_0000_0000_0000
                } else {
                    value.to_bits()
                };
                out.raw(&bits.to_le_bytes())
            }
            Value::Timestamp {
                epoch_seconds,
                nanos,
            } => {
                if *nanos >= 1_000_000_000 {
                    return Err(Self::shape("Kryo Timestamp nanos"));
                }
                let millis = i64::try_from(
                    i128::from(*epoch_seconds) * 1000 + i128::from(nanos / 1_000_000),
                )
                .map_err(|_| Self::shape("Kryo Timestamp millis"))?;
                out.class(Class::Timestamp)?;
                out.fresh()?;
                out.u64(millis as u64)?;
                out.u32(*nanos)
            }
            _ => Err(UndoCodecError::Unsupported {
                kind: "Kryo Undo field value",
            }),
        }
    }

    fn read_log(input: &mut KryoReader<'_>) -> Result<JacksonSqlUndoLog, UndoCodecError> {
        input.fresh(3)?;
        let after = Self::read_image(input)?;
        let before = Self::read_image(input)?;
        let kind = match input.byte()? {
            0 => None,
            1 => {
                let ordinal = input
                    .numeric(KryoReader::u32)?
                    .checked_sub(1)
                    .ok_or(Self::shape("Kryo SQL type"))?;
                Some(
                    SQL_TYPE_LABELS
                        .get(ordinal as usize)
                        .ok_or(Self::shape("Kryo SQL type"))?
                        .to_string(),
                )
            }
            _ => return Err(Self::shape("Kryo SQL type presence")),
        };
        let table = input.string_ref(4)?;
        Ok(JacksonSqlUndoLog::new(kind, table, before, after))
    }

    fn read_image(
        input: &mut KryoReader<'_>,
    ) -> Result<Option<JacksonUndoRecords>, UndoCodecError> {
        let empty = match input.class()? {
            None => return Ok(None),
            Some(Class::Records) => false,
            Some(Class::EmptyRecords) => true,
            _ => return Err(Self::shape("Kryo image class")),
        };
        input.fresh(4)?;
        let count = input
            .list(5, 1024, Class::Row)?
            .ok_or(Self::shape("Kryo rows list"))?;
        if empty && count != 0 {
            return Err(Self::shape("Kryo empty records with rows"));
        }
        let rows = (0..count)
            .map(|_| {
                input.fresh(6)?;
                let count = input
                    .list(7, 128, Class::Field)?
                    .ok_or(Self::shape("Kryo fields list"))?;
                (0..count).map(|_| Self::read_field(input)).collect()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let table = input.string_ref(5)?;
        let records = if empty {
            JacksonUndoRecords::empty(table)
        } else {
            JacksonUndoRecords::new(table, rows)
        };
        Ok(Some(records))
    }

    fn read_field(input: &mut KryoReader<'_>) -> Result<JacksonUndoField, UndoCodecError> {
        input.fresh(8)?;
        if input.byte()? != 1 {
            return Err(Self::shape("Kryo key type presence"));
        }
        let primary = match input.numeric(KryoReader::u32)? {
            1 => false,
            2 => true,
            _ => return Err(Self::shape("Kryo key type")),
        };
        let name = input.string_ref(9)?.ok_or(Self::shape("Kryo field name"))?;
        let jdbc = input.numeric(KryoReader::i32)?;
        let class = input.class()?;
        if !matches!(class, Some(Class::String | Class::Timestamp)) {
            input.node(9)?;
        }
        let value = match class {
            None => Value::Common(Common::Null),
            Some(Class::Integer) => Value::Common(Common::Integer(input.numeric(KryoReader::i32)?)),
            Some(Class::Long) => Value::Common(Common::Long(input.numeric(KryoReader::i64)?)),
            Some(Class::Boolean) => match input.numeric(KryoReader::byte)? {
                0 => Value::Common(Common::Boolean(false)),
                1 => Value::Common(Common::Boolean(true)),
                _ => return Err(Self::shape("Kryo boolean value")),
            },
            Some(Class::String) => Value::Common(Common::Text(
                input
                    .string_ref(9)?
                    .ok_or(Self::shape("Kryo string value"))?,
            )),
            Some(Class::Double) => {
                Value::Double(f64::from_bits(input.numeric(KryoReader::fixed64)?))
            }
            Some(Class::Timestamp) => {
                input.fresh(9)?;
                let (millis, nanos) =
                    input.numeric(|input| Ok((input.u64()? as i64, input.u32()?)))?;
                if nanos >= 1_000_000_000 {
                    return Err(Self::shape("Kryo Timestamp nanos"));
                }
                Value::Timestamp {
                    epoch_seconds: millis.div_euclid(1000),
                    nanos,
                }
            }
            _ => {
                return Err(UndoCodecError::Unsupported {
                    kind: "Kryo Undo field value",
                })
            }
        };
        Ok(JacksonUndoField::new(name, jdbc, primary, value))
    }

    fn shape(reason: &'static str) -> UndoCodecError {
        UndoCodecError::Shape { reason }
    }
}
