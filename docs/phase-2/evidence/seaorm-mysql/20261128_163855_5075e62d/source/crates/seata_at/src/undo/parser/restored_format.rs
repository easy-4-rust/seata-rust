use super::{MySqlJdbcProfile, PreparedFormat, UndoCodecError, UndoLogDocument};
use crate::{SqlType, SqlUndoLog};
use base64::{engine::general_purpose::STANDARD, Engine};
use seata_datasource::{
    ColumnMeta, ColumnType, DecimalValue, Field, FieldValue, Row, TableMeta, TableRecords,
};
use serde_json::Value;

type RestoreField = fn(&Value, &ColumnMeta, MySqlJdbcProfile) -> Result<Field, UndoCodecError>;

/// 将已检查文档与当前完整物理元数据绑定；不访问数据库、不把文档宽松校验当作执行认证。
/// 对应 Java: SQLUndoLog#setTableMeta及AbstractUndoLogManager#undo的Rust类型恢复扩展。
pub(crate) struct RestoredFormat;
impl RestoredFormat {
    pub(crate) fn build(
        document: &UndoLogDocument,
        xid: &str,
        branch_id: i64,
        tables: &[TableMeta],
        profile: MySqlJdbcProfile,
    ) -> Result<Vec<SqlUndoLog>, UndoCodecError> {
        Self::build_with(document, xid, branch_id, tables, profile, Self::field)
    }

    pub(crate) fn build_with(
        document: &UndoLogDocument,
        xid: &str,
        branch_id: i64,
        tables: &[TableMeta],
        profile: MySqlJdbcProfile,
        decode_field: RestoreField,
    ) -> Result<Vec<SqlUndoLog>, UndoCodecError> {
        let items = Self::logs_for_branch(document, xid, branch_id)?;
        let mut logs = Vec::with_capacity(items.len());
        let mut total = 0usize;
        for value in items {
            let name = value
                .get("tableName")
                .and_then(Value::as_str)
                .ok_or(Self::bad("restored table name"))?;
            let table = Self::physical_table(tables, name)?;
            let kind = match value.get("sqlType").and_then(Value::as_str) {
                Some("INSERT") => SqlType::Insert,
                Some("UPDATE") => SqlType::Update,
                Some("DELETE") => SqlType::Delete,
                _ => return Err(Self::bad("restored SQL type")),
            };
            let before = Self::image(
                value
                    .get("beforeImage")
                    .ok_or(Self::bad("restored before image"))?,
                table,
                profile,
                decode_field,
            )?;
            let after = Self::image(
                value
                    .get("afterImage")
                    .ok_or(Self::bad("restored after image"))?,
                table,
                profile,
                decode_field,
            )?;
            logs.push(Self::checked_log(kind, before, after, &mut total)?);
        }
        Ok(logs)
    }
    pub(super) fn physical_table<'a>(
        tables: &'a [TableMeta],
        name: &str,
    ) -> Result<&'a TableMeta, UndoCodecError> {
        let mut matches = tables.iter().filter(|table| table.name() == name);
        let table = matches
            .next()
            .ok_or(Self::bad("restored physical table missing"))?;
        if matches.next().is_some() {
            return Err(Self::bad("restored physical table duplicate"));
        }
        Ok(table)
    }

    pub(super) fn checked_log(
        kind: SqlType,
        before: TableRecords,
        after: TableRecords,
        total: &mut usize,
    ) -> Result<SqlUndoLog, UndoCodecError> {
        let log = SqlUndoLog::new(kind, before, after)
            .map_err(|_| Self::bad("restored image direction/identity"))?;
        *total = total
            .checked_add(log.value_bytes())
            .ok_or(Self::bad("restored payload overflow"))?;
        if *total > 16_777_216 {
            return Err(UndoCodecError::Budget {
                limit: "restored pending Undo bytes",
            });
        }
        Ok(log)
    }

    pub(crate) fn logs_for_branch<'a>(
        document: &'a UndoLogDocument,
        xid: &str,
        branch_id: i64,
    ) -> Result<&'a [Value], UndoCodecError> {
        if xid.is_empty()
            || xid.len() > 128
            || branch_id <= 0
            || document.xid() != Some(xid)
            || document.branch_id() != branch_id
        {
            return Err(Self::bad("restored original branch identity"));
        }
        document
            .sql_undo_logs()
            .filter(|v| !v.is_empty() && v.len() <= 128)
            .ok_or(Self::bad("restored nonempty log collection"))
    }

    fn bad(reason: &'static str) -> UndoCodecError {
        UndoCodecError::Shape { reason }
    }
    fn collection(value: &Value) -> Result<&[Value], UndoCodecError> {
        value
            .as_array()
            .and_then(|v| v.get(1))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .ok_or(Self::bad("restored concrete collection"))
    }
    fn image(
        value: &Value,
        table: &TableMeta,
        profile: MySqlJdbcProfile,
        decode_field: RestoreField,
    ) -> Result<TableRecords, UndoCodecError> {
        if value.get("tableName").and_then(Value::as_str) != Some(table.name()) {
            return Err(Self::bad("restored image table identity"));
        }
        let mut records = TableRecords::new(table.clone());
        let rows = Self::collection(value.get("rows").ok_or(Self::bad("restored image rows"))?)?;
        if value.get("@class").and_then(Value::as_str)
            == Some("org.apache.seata.rm.datasource.sql.struct.TableRecords$EmptyTableRecords")
            && !rows.is_empty()
        {
            return Err(Self::bad("restored EmptyTableRecords has rows"));
        }
        for raw in rows {
            let fields =
                Self::collection(raw.get("fields").ok_or(Self::bad("restored row fields"))?)?;
            if fields.len() != table.columns().len() {
                return Err(Self::bad("restored complete physical columns"));
            }
            let values = fields
                .iter()
                .zip(table.columns())
                .map(|(field, column)| decode_field(field, column, profile))
                .collect::<Result<Vec<_>, _>>()?;
            records
                .add(Row::new(table, values).map_err(|_| Self::bad("restored physical row"))?)
                .map_err(|_| Self::bad("restored row budget"))?;
        }
        Ok(records)
    }
    fn field(
        raw: &Value,
        column: &ColumnMeta,
        profile: MySqlJdbcProfile,
    ) -> Result<Field, UndoCodecError> {
        let raw_value = raw.get("value").ok_or(Self::bad("restored field value"))?;
        let payload = if let Some(array) = raw_value.as_array() {
            array.get(1).ok_or(Self::bad("restored typed value"))?
        } else {
            raw_value
        };
        if payload.is_null() && !raw_value.is_null() {
            return Err(Self::bad("restored typed null value"));
        }
        let field = Self::native_field(payload, column)?;
        let mut expected = PreparedFormat::field(&field, profile)?;
        // BigDecimal的指数与普通写法可以不同；先精确解析保留scale，再核对原Java类和JDBC声明。
        if column.kind() == ColumnType::Decimal && !raw_value.is_null() {
            if raw_value
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                != Some("java.math.BigDecimal")
            {
                return Err(Self::bad("restored decimal class"));
            }
            expected["value"] = raw_value.clone();
        }
        if &expected != raw {
            return Err(Self::bad("restored field name/key/JDBC type/value class"));
        }
        Ok(field)
    }
    pub(crate) fn native_field(
        payload: &Value,
        column: &ColumnMeta,
    ) -> Result<Field, UndoCodecError> {
        let value = if payload.is_null() {
            FieldValue::Null
        } else {
            match column.kind() {
                ColumnType::LocalDate => FieldValue::LocalDate(
                    seata_datasource::LocalDate::parse_sql(
                        payload.as_str().ok_or(Self::bad("DATE String payload"))?,
                    )
                    .map_err(|_| Self::bad("DATE calendar"))?,
                ),
                ColumnType::InternalTime => FieldValue::InternalTime(
                    seata_datasource::InternalTime::parse_sql(
                        payload.as_str().ok_or(Self::bad("TIME String payload"))?,
                    )
                    .map_err(|_| Self::bad("TIME signed microseconds"))?,
                ),
                ColumnType::Timestamp => {
                    let (millis, nanos) = if let Some(a) = payload.as_array() {
                        if a.len() != 2 {
                            return Err(Self::bad("TIMESTAMP timestamp shape"));
                        }
                        let millis = a[0].as_i64().ok_or(Self::bad("TIMESTAMP millis"))?;
                        let nanos = a[1]
                            .as_u64()
                            .and_then(|n| u32::try_from(n).ok())
                            .ok_or(Self::bad("TIMESTAMP nanos"))?;
                        (millis, nanos)
                    } else {
                        let millis = payload.as_i64().ok_or(Self::bad("TIMESTAMP millis"))?;
                        (millis, millis.rem_euclid(1000) as u32 * 1_000_000)
                    };
                    // 原 Timestamp#setNanos 覆盖完整秒内纳秒，不能采用 LocalDateTime 的仅追加余数规则。
                    FieldValue::Timestamp(
                        seata_datasource::Timestamp::from_epoch_seconds(
                            millis.div_euclid(1000),
                            nanos,
                        )
                        .map_err(|_| Self::bad("TIMESTAMP epoch or precision"))?,
                    )
                }
                ColumnType::LocalDateTime => {
                    let (millis, extra) = if let Some(a) = payload.as_array() {
                        if a.len() != 2 {
                            return Err(Self::bad("DATETIME timestamp shape"));
                        }
                        let nanos = a[1]
                            .as_u64()
                            .and_then(|n| u32::try_from(n).ok())
                            .filter(|n| *n < 1_000_000_000)
                            .ok_or(Self::bad("DATETIME nanos"))?;
                        (
                            a[0].as_i64().ok_or(Self::bad("DATETIME millis"))?,
                            nanos % 1_000_000,
                        )
                    } else {
                        (payload.as_i64().ok_or(Self::bad("DATETIME millis"))?, 0)
                    };
                    let nanos = (millis.rem_euclid(1000) as u32) * 1_000_000 + extra;
                    FieldValue::LocalDateTime(
                        seata_datasource::LocalDateTime::from_epoch_seconds(
                            millis.div_euclid(1000),
                            nanos,
                        )
                        .map_err(|_| Self::bad("DATETIME calendar"))?,
                    )
                }
                ColumnType::SignedInteger => {
                    FieldValue::Signed(if let Some(v) = payload.as_bool() {
                        i64::from(v)
                    } else {
                        payload
                            .as_i64()
                            .ok_or(Self::bad("restored signed integer"))?
                    })
                }
                ColumnType::UnsignedInteger => FieldValue::Unsigned(
                    payload
                        .as_u64()
                        .ok_or(Self::bad("restored unsigned integer"))?,
                ),
                ColumnType::Text => FieldValue::Text(
                    payload
                        .as_str()
                        .ok_or(Self::bad("restored native text"))?
                        .to_owned(),
                ),
                ColumnType::Binary => FieldValue::Binary(
                    STANDARD
                        .decode(
                            payload
                                .as_str()
                                .ok_or(Self::bad("restored native binary"))?,
                        )
                        .map_err(|_| Self::bad("restored binary base64"))?,
                ),
                ColumnType::Decimal => FieldValue::Decimal(Self::decimal(
                    &payload
                        .as_number()
                        .ok_or(Self::bad("restored decimal number"))?
                        .to_string(),
                )?),
            }
        };
        Field::new(column, value).map_err(|_| Self::bad("restored physical value domain"))
    }

    pub(super) fn decimal(text: &str) -> Result<DecimalValue, UndoCodecError> {
        let (mantissa, exponent) = match text.split_once(['e', 'E']) {
            Some((m, e)) => (
                m,
                e.parse::<i32>()
                    .map_err(|_| Self::bad("restored decimal exponent"))?,
            ),
            None => (text, 0),
        };
        if !(-95..=95).contains(&exponent) {
            return Err(Self::bad("restored decimal capacity"));
        }
        let negative = mantissa.starts_with('-');
        let unsigned = mantissa.strip_prefix('-').unwrap_or(mantissa);
        let fraction = unsigned.split_once('.').map_or(0, |(_, f)| f.len());
        let scale =
            i32::try_from(fraction).map_err(|_| Self::bad("restored decimal scale"))? - exponent;
        if !(0..=30).contains(&scale) {
            return Err(Self::bad("restored physical decimal scale"));
        }
        let mut digits = unsigned.replace('.', "");
        if digits.len() > 96 {
            return Err(Self::bad("restored decimal digits"));
        }
        if scale < 0 {
            digits.push_str(&"0".repeat((-scale) as usize));
        }
        let scale = scale.max(0) as usize;
        if scale >= digits.len() {
            digits = format!("{}{}", "0".repeat(scale + 1 - digits.len()), digits);
        }
        let position = digits.len() - scale;
        let integer = digits[..position].trim_start_matches('0');
        let integer = if integer.is_empty() { "0" } else { integer };
        let mut plain = integer.to_owned();
        if scale > 0 {
            plain.push('.');
            plain.push_str(&digits[position..]);
        }
        if negative && digits.bytes().any(|b| b != b'0') {
            plain.insert(0, '-');
        }
        DecimalValue::new(&plain).map_err(|_| Self::bad("restored exact decimal"))
    }
}
