use super::{MySqlJdbcProfile, UndoCodecError, UndoCodecLimits};
use crate::{SqlType, SqlUndoLog};
use base64::{engine::general_purpose::STANDARD, Engine};
use seata_datasource::{ColumnDomain, ColumnMeta, Field, FieldValue, MySqlType, TableRecords};
use serde_json::{Number, Value};
use std::str::FromStr;

/// 从已检查的本地准备镜像生成原Java对象图，不登记分支、不写库、不提交。
/// 对应 Java: JacksonUndoLogParser#encode、BaseTransactionalExecutor#prepareUndoLog的Rust边界扩展。
pub(crate) struct PreparedFormat;
/// 原生值的自然、无损Boolean或显式Java类包装方式；对应Jackson值类型标签。
enum ValueEncoding {
    Natural,
    Boolean,
    Typed(&'static str),
}
impl PreparedFormat {
    pub(crate) fn build(
        xid: &str,
        branch_id: i64,
        logs: &[SqlUndoLog],
        profile: MySqlJdbcProfile,
        limits: UndoCodecLimits,
    ) -> Result<Value, UndoCodecError> {
        if xid.is_empty() || xid.len() > 128 || branch_id <= 0 {
            return Err(Self::shape("prepared branch arguments"));
        }
        if logs.len() > 128 {
            return Err(UndoCodecError::Budget {
                limit: "prepared log count",
            });
        }
        // 必要条件预检避免先克隆任意大的对象图；最终完整图再精确检查节点/scalar及输出。
        let mut minimum_nodes = 7usize;
        let mut payload = 0usize;
        for log in logs {
            Self::add(&mut minimum_nodes, 16, limits.nodes, "prepared nodes")?;
            Self::add(
                &mut payload,
                log.value_bytes(),
                limits.total_scalar,
                "prepared payload",
            )?;
            for image in [log.before_image(), log.after_image()] {
                for row in image.rows() {
                    let nodes = row
                        .fields()
                        .len()
                        .checked_mul(6)
                        .and_then(|n| n.checked_add(5))
                        .ok_or(UndoCodecError::Budget {
                            limit: "prepared nodes overflow",
                        })?;
                    Self::add(&mut minimum_nodes, nodes, limits.nodes, "prepared nodes")?;
                }
            }
        }
        let mut items = Vec::with_capacity(logs.len());
        for log in logs {
            let kind = match log.sql_type() {
                SqlType::Insert => "INSERT",
                SqlType::Update => "UPDATE",
                SqlType::Delete => "DELETE",
            };
            let before = Self::image(
                log.before_image(),
                log.sql_type() == SqlType::Insert,
                profile,
            )?;
            let after = Self::image(
                log.after_image(),
                log.sql_type() == SqlType::Delete,
                profile,
            )?;
            items.push(serde_json::json!({"@class":"org.apache.seata.rm.datasource.undo.SQLUndoLog","sqlType":kind,
                "tableName":log.table_name(),"beforeImage":before,"afterImage":after}));
        }
        Ok(
            serde_json::json!({"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","xid":xid,
            "branchId":branch_id,"sqlUndoLogs":Self::list(items)}),
        )
    }
    fn add(
        total: &mut usize,
        value: usize,
        limit: usize,
        label: &'static str,
    ) -> Result<(), UndoCodecError> {
        *total = total
            .checked_add(value)
            .ok_or(UndoCodecError::Budget { limit: label })?;
        if *total > limit {
            Err(UndoCodecError::Budget { limit: label })
        } else {
            Ok(())
        }
    }
    fn list(values: Vec<Value>) -> Value {
        Value::Array(vec![
            Value::String("java.util.ArrayList".into()),
            Value::Array(values),
        ])
    }
    fn shape(reason: &'static str) -> UndoCodecError {
        UndoCodecError::Shape { reason }
    }
    fn image(
        image: &TableRecords,
        empty: bool,
        profile: MySqlJdbcProfile,
    ) -> Result<Value, UndoCodecError> {
        let class = if empty {
            "org.apache.seata.rm.datasource.sql.struct.TableRecords$EmptyTableRecords"
        } else {
            "org.apache.seata.rm.datasource.sql.struct.TableRecords"
        };
        let mut rows = Vec::with_capacity(image.rows().len());
        for row in image.rows() {
            let fields = row
                .fields()
                .iter()
                .map(|field| Self::field(field, profile))
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(serde_json::json!({"@class":"org.apache.seata.rm.datasource.sql.struct.Row","fields":Self::list(fields)}));
        }
        Ok(
            serde_json::json!({"@class":class,"tableName":image.table_name(),"rows":Self::list(rows)}),
        )
    }
    pub(super) fn field(field: &Field, profile: MySqlJdbcProfile) -> Result<Value, UndoCodecError> {
        let (code, encoding) = Self::descriptor(field.column(), profile)?;
        let value = if matches!(field.value(), FieldValue::Null) {
            Value::Null
        } else {
            let payload = match field.value() {
                FieldValue::LocalDate(value) => Value::String(value.to_sql_string()),
                FieldValue::InternalTime(value) => {
                    let Some(ColumnDomain::InternalTime { fractional_seconds }) =
                        field.column().domain()
                    else {
                        return Err(Self::shape("TIME physical precision"));
                    };
                    Value::String(
                        value
                            .to_sql_string(*fractional_seconds)
                            .map_err(|_| Self::shape("TIME physical precision"))?,
                    )
                }
                FieldValue::Timestamp(value) => {
                    let millis =
                        value.epoch_seconds() * 1000 + i64::from(value.nanosecond() / 1_000_000);
                    if value.nanosecond() % 1_000_000 == 0 {
                        Value::Number(millis.into())
                    } else {
                        serde_json::json!([millis, value.nanosecond()])
                    }
                }
                FieldValue::LocalDateTime(value) => {
                    let millis =
                        value.epoch_seconds() * 1000 + i64::from(value.nanosecond() / 1_000_000);
                    if value.nanosecond() % 1_000_000 == 0 {
                        Value::Number(millis.into())
                    } else {
                        serde_json::json!([millis, value.nanosecond()])
                    }
                }
                FieldValue::Null => Value::Null,
                FieldValue::Signed(value) => Value::Number((*value).into()),
                FieldValue::Unsigned(value) => Value::Number((*value).into()),
                FieldValue::Text(value) => Value::String(value.clone()),
                FieldValue::Binary(value) => Value::String(STANDARD.encode(value)),
                FieldValue::Decimal(value) => Value::Number(
                    Number::from_str(value.as_str())
                        .map_err(|_| Self::shape("prepared decimal"))?,
                ),
            };
            match encoding {
                ValueEncoding::Natural => payload,
                ValueEncoding::Typed(class) => {
                    Value::Array(vec![Value::String(class.into()), payload])
                }
                ValueEncoding::Boolean => match field.value() {
                    FieldValue::Signed(0) => Value::Bool(false),
                    FieldValue::Signed(1) => Value::Bool(true),
                    _ => {
                        return Err(UndoCodecError::Unsupported {
                            kind: "lossy tinyint Boolean conversion",
                        })
                    }
                },
            }
        };
        let key = if field.is_primary_key() {
            "PRIMARY_KEY"
        } else {
            "NULL"
        };
        Ok(
            serde_json::json!({"@class":"org.apache.seata.rm.datasource.sql.struct.Field","name":field.name(),"keyType":key,"type":code,"value":value}),
        )
    }
    pub(super) fn physical_descriptor(
        column: &ColumnMeta,
        profile: MySqlJdbcProfile,
    ) -> Result<(i32, &'static str), UndoCodecError> {
        let (code, encoding) = Self::descriptor(column, profile)?;
        let class = match encoding {
            ValueEncoding::Typed(class) => class,
            ValueEncoding::Boolean => "java.lang.Boolean",
            ValueEncoding::Natural
                if matches!(
                    column.kind(),
                    seata_datasource::ColumnType::Text
                        | seata_datasource::ColumnType::LocalDate
                        | seata_datasource::ColumnType::InternalTime
                ) =>
            {
                "java.lang.String"
            }
            ValueEncoding::Natural => "java.lang.Integer",
        };
        Ok((code, class))
    }

    fn descriptor(
        column: &ColumnMeta,
        profile: MySqlJdbcProfile,
    ) -> Result<(i32, ValueEncoding), UndoCodecError> {
        let native = column.mysql_type().ok_or(UndoCodecError::Unsupported {
            kind: "missing native type identity",
        })?;
        let unsigned = matches!(
            column.domain(),
            Some(ColumnDomain::Integer { unsigned: true, .. })
        );
        Ok(match native {
            MySqlType::Date | MySqlType::Time => {
                if !profile.exact_date_time() {
                    return Err(UndoCodecError::Unsupported {
                        kind: "explicit exact DATE/TIME String profile required",
                    });
                }
                (
                    if native == MySqlType::Date { 91 } else { 92 },
                    ValueEncoding::Natural,
                )
            }
            MySqlType::Timestamp => {
                if !profile.utc_temporal() {
                    return Err(UndoCodecError::Unsupported {
                        kind: "explicit UTC TIMESTAMP profile required",
                    });
                }
                (93, ValueEncoding::Typed("java.sql.Timestamp"))
            }
            MySqlType::DateTime => {
                if !profile.utc_temporal() {
                    return Err(UndoCodecError::Unsupported {
                        kind: "explicit UTC DATETIME profile required",
                    });
                }
                (93, ValueEncoding::Typed("java.time.LocalDateTime"))
            }
            MySqlType::TinyInt {
                display_width_one: true,
            } if !unsigned && profile.boolean_tinyint() => (-7, ValueEncoding::Boolean),
            MySqlType::TinyInt { .. } => (-6, ValueEncoding::Natural),
            MySqlType::SmallInt => (5, ValueEncoding::Natural),
            MySqlType::MediumInt => (4, ValueEncoding::Natural),
            MySqlType::Int if unsigned => (4, ValueEncoding::Typed("java.lang.Long")),
            MySqlType::Int => (4, ValueEncoding::Natural),
            MySqlType::BigInt if unsigned => (-5, ValueEncoding::Typed("java.math.BigInteger")),
            MySqlType::BigInt => (-5, ValueEncoding::Typed("java.lang.Long")),
            MySqlType::Decimal => (3, ValueEncoding::Typed("java.math.BigDecimal")),
            MySqlType::Char => (1, ValueEncoding::Natural),
            MySqlType::VarChar | MySqlType::TinyText => (12, ValueEncoding::Natural),
            MySqlType::Json | MySqlType::Text | MySqlType::MediumText | MySqlType::LongText => {
                (-1, ValueEncoding::Natural)
            }
            MySqlType::Binary => (-2, ValueEncoding::Typed("[B")),
            MySqlType::VarBinary | MySqlType::TinyBlob => (-3, ValueEncoding::Typed("[B")),
            MySqlType::Blob | MySqlType::MediumBlob | MySqlType::LongBlob => {
                (-4, ValueEncoding::Typed("[B"))
            }
        })
    }
}
