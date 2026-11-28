use super::{
    bounded_output::BoundedOutput,
    protostuff_reader::ProtostuffReader,
    protostuff_value::{ProtostuffFields, ProtostuffValue as Wire},
    protostuff_writer::ProtostuffWriter,
    restored_format::RestoredFormat,
    sql_type_labels::SQL_TYPE_LABELS,
    FastjsonUndoFieldValue as Common, JacksonSqlUndoLog, JacksonUndoField,
    JacksonUndoFieldValue as FieldValue, JacksonUndoRecords, ProtostuffUndoBranch, UndoCodecError,
    UndoCodecLimits,
};

const TIMESTAMP: &[u8] = b"java.sql.Timestamp";

/// Protostuff 的原 Undo 有界编解码器，支持分组结构与固定 Timestamp 委托。
/// 对应 Java: org.apache.seata.rm.datasource.undo.parser.ProtostuffUndoLogParser。
#[derive(Debug, Default, Clone)]
pub struct ProtostuffUndoLogParser {
    limits: UndoCodecLimits,
}

impl ProtostuffUndoLogParser {
    /// 参数为校验过的结构与字节预算，返回独立对象编解码器。
    pub fn new(limits: UndoCodecLimits) -> Self {
        Self { limits }
    }

    /// 无参数；返回 Java 默认分支编码，包含零分支号。
    pub fn default_content(&self) -> Result<Vec<u8>, UndoCodecError> {
        self.encode_branch(None, 0, None)
    }

    /// 参数为原 XID、分支号及可空日志列表，返回有界 Protostuff 字节。
    /// 对应 Java: ProtostuffUndoLogParser#encode。
    pub fn encode_branch(
        &self,
        xid: Option<&str>,
        branch_id: i64,
        logs: Option<&[JacksonSqlUndoLog]>,
    ) -> Result<Vec<u8>, UndoCodecError> {
        let bytes = BoundedOutput::generate(self.limits.encoded, |out| {
            let mut out = ProtostuffWriter(out);
            Self::write_text(&mut out, 1, xid)?;
            out.number(2, branch_id as u64)?;
            for log in logs.into_iter().flatten() {
                out.group(3, |out| Self::write_log(out, log))?;
            }
            Ok(())
        })?;
        self.decode_branch(&bytes)?;
        Ok(bytes)
    }

    /// 参数为完整字节，返回原分支与受支持的原字段类型。
    /// 对应 Java: ProtostuffUndoLogParser#decode；空输入产生默认分支。
    pub fn decode_branch(&self, bytes: &[u8]) -> Result<ProtostuffUndoBranch, UndoCodecError> {
        let mut fields = ProtostuffReader::parse(bytes, self.limits)?;
        let xid = Self::text(Self::single(&mut fields, 1)?)?;
        let branch_id = Self::number(Self::single(&mut fields, 2)?)?.unwrap_or_default() as i64;
        let logs = Self::groups(&mut fields, 3, 128, Self::read_log)?;
        Self::complete(&fields)?;
        let logs = (!logs.is_empty()).then_some(logs);
        Ok(ProtostuffUndoBranch {
            xid,
            branch_id,
            logs,
        })
    }

    fn write_log(
        out: &mut ProtostuffWriter<'_>,
        log: &JacksonSqlUndoLog,
    ) -> Result<(), UndoCodecError> {
        if let Some(kind) = log.sql_type() {
            let ordinal = SQL_TYPE_LABELS
                .iter()
                .position(|label| *label == kind)
                .ok_or(Self::shape("Protostuff SQL type"))?;
            out.number(1, ordinal as u64)?;
        }
        Self::write_text(out, 2, log.table_name())?;
        for (tag, image) in [(3, log.before_image()), (4, log.after_image())] {
            if let Some(image) = image {
                out.group(tag, |out| Self::write_image(out, image))?;
            }
        }
        Ok(())
    }

    fn write_image(
        out: &mut ProtostuffWriter<'_>,
        image: &JacksonUndoRecords,
    ) -> Result<(), UndoCodecError> {
        Self::write_text(out, 1, image.table_name())?;
        for row in image.rows() {
            out.group(2, |out| {
                for field in row {
                    out.group(1, |out| Self::write_field(out, field))?;
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    fn write_field(
        out: &mut ProtostuffWriter<'_>,
        field: &JacksonUndoField,
    ) -> Result<(), UndoCodecError> {
        out.bytes(1, field.name.as_bytes())?;
        out.number(2, u64::from(field.primary))?;
        out.number(3, i64::from(field.jdbc_type) as u64)?;
        if matches!(field.value(), FieldValue::Common(Common::Null)) {
            return Ok(());
        }
        out.group(4, |out| Self::write_value(out, field.value()))
    }

    fn write_value(
        out: &mut ProtostuffWriter<'_>,
        value: &FieldValue,
    ) -> Result<(), UndoCodecError> {
        match value {
            FieldValue::Common(Common::Boolean(value)) => out.number(1, u64::from(*value)),
            FieldValue::Common(Common::Integer(value)) => out.number(5, i64::from(*value) as u64),
            FieldValue::Common(Common::Long(value)) => out.number(6, *value as u64),
            FieldValue::Double(value) => out.fixed64(8, value.to_bits()),
            FieldValue::Common(Common::Text(value)) => out.bytes(9, value.as_bytes()),
            FieldValue::Common(Common::Binary(bytes)) => out.bytes(11, bytes),
            FieldValue::Common(Common::Decimal(value)) => {
                out.bytes(12, value.java_big_decimal_text().as_bytes())
            }
            FieldValue::Common(Common::Unsigned(value)) => {
                // BigInteger#toByteArray 使用最短有符号大端编码，正数可能需要前导零。
                let mut bytes = [0; 9];
                bytes[1..].copy_from_slice(&value.to_be_bytes());
                let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(8);
                let start = first - usize::from(bytes[first] >= 128);
                out.bytes(13, &bytes[start..])
            }
            FieldValue::Timestamp {
                epoch_seconds,
                nanos,
            } => {
                if *nanos >= 1_000_000_000 {
                    return Err(Self::shape("Protostuff Timestamp nanos"));
                }
                let millis = i64::try_from(
                    i128::from(*epoch_seconds) * 1000 + i128::from(nanos / 1_000_000),
                )
                .map_err(|_| Self::shape("Protostuff Timestamp millis"))?;
                let mut delegate = [0; 12];
                delegate[..8].copy_from_slice(&millis.to_be_bytes());
                delegate[8..].copy_from_slice(&nanos.to_be_bytes());
                out.bytes(30, TIMESTAMP)?;
                out.bytes(1, &delegate)
            }
            _ => Err(UndoCodecError::Unsupported {
                kind: "Protostuff Undo field value",
            }),
        }
    }

    fn write_text(
        out: &mut ProtostuffWriter<'_>,
        field: u32,
        value: Option<&str>,
    ) -> Result<(), UndoCodecError> {
        if let Some(value) = value {
            out.bytes(field, value.as_bytes())?;
        }
        Ok(())
    }

    fn read_log(mut fields: ProtostuffFields<'_>) -> Result<JacksonSqlUndoLog, UndoCodecError> {
        let kind = Self::number(Self::single(&mut fields, 1)?)?
            .map(|number| {
                let index =
                    usize::try_from(number).map_err(|_| Self::shape("Protostuff SQL type"))?;
                SQL_TYPE_LABELS
                    .get(index)
                    .map(|label| label.to_string())
                    .ok_or(Self::shape("Protostuff SQL type"))
            })
            .transpose()?;
        let table = Self::text(Self::single(&mut fields, 2)?)?;
        let before = Self::group(Self::single(&mut fields, 3)?)?
            .map(Self::read_image)
            .transpose()?;
        let after = Self::group(Self::single(&mut fields, 4)?)?
            .map(Self::read_image)
            .transpose()?;
        Self::complete(&fields)?;
        Ok(JacksonSqlUndoLog::new(kind, table, before, after))
    }

    fn read_image(mut fields: ProtostuffFields<'_>) -> Result<JacksonUndoRecords, UndoCodecError> {
        let table = Self::text(Self::single(&mut fields, 1)?)?;
        let rows = Self::groups(&mut fields, 2, 1024, |mut row| {
            let values = Self::groups(&mut row, 1, 128, Self::read_field)?;
            Self::complete(&row)?;
            Ok(values)
        })?;
        Self::complete(&fields)?;
        // TableRecords 的声明 schema 不在线上保留子类标签。
        Ok(JacksonUndoRecords::new(table, rows))
    }

    fn read_field(mut fields: ProtostuffFields<'_>) -> Result<JacksonUndoField, UndoCodecError> {
        let name = Self::text(Self::single(&mut fields, 1)?)?
            .ok_or(Self::shape("Protostuff field name"))?;
        let primary =
            Self::boolean(Self::number(Self::single(&mut fields, 2)?)?.unwrap_or_default())?;
        let jdbc = Self::int32(Self::number(Self::single(&mut fields, 3)?)?.unwrap_or_default())?;
        let value = Self::group(Self::single(&mut fields, 4)?)?
            .map(Self::read_value)
            .transpose()?
            .unwrap_or(FieldValue::Common(Common::Null));
        Self::complete(&fields)?;
        Ok(JacksonUndoField::new(name, jdbc, primary, value))
    }

    fn read_value(mut fields: ProtostuffFields<'_>) -> Result<FieldValue, UndoCodecError> {
        if let Some(class) = Self::single(&mut fields, 30)? {
            if fields.first != Some(30) {
                return Err(Self::shape("Protostuff delegate discriminator order"));
            }
            if !matches!(class, Wire::Bytes(bytes) if bytes == TIMESTAMP) {
                return Err(UndoCodecError::Unsupported {
                    kind: "Protostuff delegate class",
                });
            }
            let Some(Wire::Bytes(bytes)) = Self::single(&mut fields, 1)? else {
                return Err(Self::shape("Protostuff Timestamp delegate"));
            };
            Self::complete(&fields)?;
            let parts: [u8; 12] = bytes
                .try_into()
                .map_err(|_| Self::shape("Protostuff Timestamp delegate length"))?;
            let millis =
                i64::from_be_bytes(parts[..8].try_into().map_err(|_| UndoCodecError::Syntax)?);
            let nanos =
                u32::from_be_bytes(parts[8..].try_into().map_err(|_| UndoCodecError::Syntax)?);
            if nanos >= 1_000_000_000 {
                return Err(Self::shape("Protostuff Timestamp nanos"));
            }
            return Ok(FieldValue::Timestamp {
                epoch_seconds: millis.div_euclid(1000),
                nanos,
            });
        }
        let (tag, mut values) = fields
            .members
            .pop_first()
            .ok_or(Self::shape("Protostuff empty field value"))?;
        let value = values
            .pop()
            .ok_or(Self::shape("Protostuff empty field value"))?;
        if !values.is_empty() || !fields.members.is_empty() {
            return Err(Self::shape("Protostuff multiple field value tags"));
        }
        let common = match (tag, value) {
            (1, Wire::Varint(value)) => Common::Boolean(Self::boolean(value)?),
            (5, Wire::Varint(value)) => Common::Integer(Self::int32(value)?),
            (6, Wire::Varint(value)) => Common::Long(value as i64),
            (8, Wire::Fixed64(value)) => return Ok(FieldValue::Double(f64::from_bits(value))),
            (9, Wire::Bytes(value)) => Common::Text(Self::utf8(value)?.into()),
            (11, Wire::Bytes(value)) => Common::Binary(value.to_vec()),
            (12, Wire::Bytes(value)) => {
                Common::Decimal(RestoredFormat::decimal(Self::utf8(value)?)?)
            }
            (13, Wire::Bytes(value)) => Common::Unsigned(Self::positive_big_int(value)?),
            _ => {
                return Err(UndoCodecError::Unsupported {
                    kind: "Protostuff Undo field value",
                })
            }
        };
        Ok(FieldValue::Common(common))
    }

    fn positive_big_int(value: &[u8]) -> Result<u64, UndoCodecError> {
        if value.is_empty() || value[0] >= 128 {
            return Err(Self::shape("Protostuff nonnegative BigInteger"));
        }
        let magnitude = if value.first() == Some(&0) {
            &value[1..]
        } else {
            value
        };
        if magnitude.len() > 8 {
            return Err(Self::shape("Protostuff BigInteger u64 range"));
        }
        let mut bytes = [0; 8];
        bytes[8 - magnitude.len()..].copy_from_slice(magnitude);
        Ok(u64::from_be_bytes(bytes))
    }

    fn groups<'a, T>(
        fields: &mut ProtostuffFields<'a>,
        tag: u32,
        max: usize,
        read: impl Fn(ProtostuffFields<'a>) -> Result<T, UndoCodecError>,
    ) -> Result<Vec<T>, UndoCodecError> {
        let values = fields.members.remove(&tag).unwrap_or_default();
        if values.len() > max {
            return Err(UndoCodecError::Budget {
                limit: "Protostuff Undo collection elements",
            });
        }
        values
            .into_iter()
            .map(|item| {
                let fields =
                    Self::group(Some(item))?.ok_or(Self::shape("Protostuff group field"))?;
                read(fields)
            })
            .collect()
    }

    fn single<'a>(
        fields: &mut ProtostuffFields<'a>,
        tag: u32,
    ) -> Result<Option<Wire<'a>>, UndoCodecError> {
        let Some(mut values) = fields.members.remove(&tag) else {
            return Ok(None);
        };
        if values.len() != 1 {
            return Err(Self::shape("duplicate Protostuff singular field"));
        }
        Ok(values.pop())
    }

    fn group(value: Option<Wire<'_>>) -> Result<Option<ProtostuffFields<'_>>, UndoCodecError> {
        match value {
            None => Ok(None),
            Some(Wire::Group(fields)) => Ok(Some(fields)),
            _ => Err(Self::shape("Protostuff group field")),
        }
    }

    fn number(value: Option<Wire<'_>>) -> Result<Option<u64>, UndoCodecError> {
        match value {
            None => Ok(None),
            Some(Wire::Varint(value)) => Ok(Some(value)),
            _ => Err(Self::shape("Protostuff varint field")),
        }
    }

    fn boolean(value: u64) -> Result<bool, UndoCodecError> {
        match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Self::shape("Protostuff boolean value")),
        }
    }

    fn int32(value: u64) -> Result<i32, UndoCodecError> {
        i32::try_from(value as i64).map_err(|_| Self::shape("Protostuff int32 range"))
    }

    fn text(value: Option<Wire<'_>>) -> Result<Option<String>, UndoCodecError> {
        match value {
            None => Ok(None),
            Some(Wire::Bytes(value)) => Ok(Some(Self::utf8(value)?.into())),
            _ => Err(Self::shape("Protostuff text field")),
        }
    }

    fn utf8(value: &[u8]) -> Result<&str, UndoCodecError> {
        std::str::from_utf8(value).map_err(|_| UndoCodecError::Syntax)
    }

    fn complete(fields: &ProtostuffFields<'_>) -> Result<(), UndoCodecError> {
        if fields.members.is_empty() {
            Ok(())
        } else {
            Err(Self::shape("unknown Protostuff Undo field"))
        }
    }

    fn shape(reason: &'static str) -> UndoCodecError {
        UndoCodecError::Shape { reason }
    }
}
