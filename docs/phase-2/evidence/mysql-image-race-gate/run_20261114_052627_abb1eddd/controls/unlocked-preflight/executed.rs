use crate::{cache::MySqlTableMetaReader, DbError};
use bigdecimal::BigDecimal;
use seata_at::{SqlRecognizer, SqlType};
use seata_datasource::{
    ColumnMeta, ColumnType, DecimalValue, Field, FieldValue, ImageError, Row as ImageRow,
    TableMeta, TableRecords,
};
use sqlx::{
    mysql::{MySqlArguments, MySqlRow},
    Arguments, MySql, Row, Transaction, ValueRef,
};
use std::str::FromStr;

/// 同事务长度预检、锁定与精确行构造；不执行业务SQL。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.TableRecords#buildRecords的MySQL读取路径（部分实现）。
pub(crate) struct MySqlImageReader;
impl MySqlImageReader {
    /// 参数为正式识别结果和完整原参数；返回本地形状/预算校验，不访问数据库。
    pub(crate) fn validate_request(
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<(), DbError> {
        if sql.sql_type() == SqlType::Insert {
            return Err(DbError::Unsupported {
                feature: "INSERT before image path not implemented",
            });
        }
        if parameters.len() != sql.parameter_count()
            || sql.predicate_parameter_start() > parameters.len()
        {
            return Err(DbError::Input {
                reason: "exact original parameter count required",
            });
        }
        let mut total = 0usize;
        for value in parameters {
            let bytes = value.value_bytes();
            total = total.checked_add(bytes).ok_or(ImageError::Budget {
                limit: "parameter bytes overflow",
            })?;
            if bytes > 1_048_576 || total > 8_388_608 {
                return Err(ImageError::Budget {
                    limit: "parameter payload",
                }
                .into());
            }
        }
        Ok(())
    }
    /// 参数为原拥有式事务、schema、识别结果及原参数；返回持锁完整前镜像，任何失败不发布部分结果。
    pub(crate) async fn read(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<TableRecords, DbError> {
        let table = MySqlTableMetaReader::fetch(tx, database, sql.table_name()).await?;
        Self::validate_columns(&table, sql)?;
        let predicate_parameters = &parameters[sql.predicate_parameter_start()..];
        let suffix = Self::suffix(&table, sql)?;
        let quoted = table
            .columns()
            .iter()
            .map(|column| TableMeta::quote_identifier(column.name()))
            .collect::<Result<Vec<_>, _>>()?;
        let lengths = table
            .columns()
            .iter()
            .zip(&quoted)
            .map(|(column, name)| {
                let expression = match column.kind() {
                    ColumnType::SignedInteger | ColumnType::UnsignedInteger => {
                        format!("CASE WHEN {name} IS NULL THEN NULL ELSE 8 END")
                    }
                    ColumnType::Decimal => {
                        format!("OCTET_LENGTH(CAST({name} AS CHAR CHARACTER SET utf8mb4))")
                    }
                    ColumnType::Text | ColumnType::Binary => format!("OCTET_LENGTH({name})"),
                };
                format!("CAST({expression} AS UNSIGNED)")
            })
            .collect::<Vec<_>>()
            .join(", ");
        // 只取长度与NULL标记，避免先把超限BLOB交给驱动分配；RR锁覆盖下一次同条件读取。
        let preflight_sql = format!("SELECT {lengths}{}", suffix.trim_end_matches(" FOR UPDATE"));
        let preflight = sqlx::query_with(&preflight_sql, Self::arguments(predicate_parameters)?)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| DbError::driver("image length preflight", e))?;
        let sizes = Self::preflight_sizes(&preflight, table.columns().len())?;
        let mut records = TableRecords::new(table.clone());
        if sizes.is_empty() {
            return Ok(records);
        }
        let values_sql = format!("SELECT {}{suffix}", quoted.join(", "));
        let values = sqlx::query_with(&values_sql, Self::arguments(predicate_parameters)?)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| DbError::driver("image values", e))?;
        if values.len() != sizes.len() {
            return Err(DbError::Input {
                reason: "locked image row count changed",
            });
        }
        for (row, lengths) in values.iter().zip(sizes) {
            let mut fields = Vec::with_capacity(table.columns().len());
            for (index, column) in table.columns().iter().enumerate() {
                let value = Self::decode(row, index, column)?;
                let actual = if matches!(value, FieldValue::Null) {
                    None
                } else {
                    Some(value.value_bytes() as u64)
                };
                if actual != lengths[index] {
                    return Err(DbError::Input {
                        reason: "locked image length/null marker changed",
                    });
                }
                fields.push(Field::new(column, value)?);
            }
            records.add(ImageRow::new(&table, fields)?)?;
        }
        Ok(records)
    }
    fn validate_columns(table: &TableMeta, sql: &SqlRecognizer) -> Result<(), DbError> {
        if table.columns().iter().any(|c| c.domain().is_none()) {
            return Err(DbError::Unsupported {
                feature: "physical column domains required",
            });
        }
        if sql.sql_type() == SqlType::Update {
            for name in sql.columns() {
                let column = table.column(name).ok_or(DbError::Input {
                    reason: "unknown assignment column",
                })?;
                if column.primary_key_ordinal().is_some() {
                    return Err(DbError::Unsupported {
                        feature: "primary key update",
                    });
                }
            }
        }
        Ok(())
    }
    fn suffix(table: &TableMeta, sql: &SqlRecognizer) -> Result<String, DbError> {
        let predicate = sql
            .predicate_sql()
            .map_or_else(String::new, |p| format!(" WHERE {p}"));
        let keys = table
            .primary_key_columns()
            .map(|column| TableMeta::quote_identifier(column.name()))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        Ok(format!(
            " FROM {}{predicate} ORDER BY {keys} LIMIT 1025 FOR UPDATE",
            TableMeta::quote_identifier(table.name())?
        ))
    }
    fn arguments(parameters: &[FieldValue]) -> Result<MySqlArguments, DbError> {
        let mut arguments = MySqlArguments::default();
        for value in parameters {
            let result = match value {
                FieldValue::Null => arguments.add(Option::<i64>::None),
                FieldValue::Signed(v) => arguments.add(*v),
                FieldValue::Unsigned(v) => arguments.add(*v),
                FieldValue::Text(v) => arguments.add(v.as_str()),
                FieldValue::Binary(v) => arguments.add(v.as_slice()),
                FieldValue::Decimal(v) => arguments.add(BigDecimal::from_str(v.as_str()).map_err(
                    |_| DbError::Input {
                        reason: "exact decimal parameter",
                    },
                )?),
            };
            result.map_err(|_| DbError::Input {
                reason: "parameter encoding",
            })?;
        }
        Ok(arguments)
    }
    fn preflight_sizes(
        rows: &[MySqlRow],
        columns: usize,
    ) -> Result<Vec<Vec<Option<u64>>>, DbError> {
        if rows.len() > 1024 {
            return Err(ImageError::Budget {
                limit: "1024 image rows",
            }
            .into());
        }
        let mut total = 0u64;
        let mut sizes = Vec::with_capacity(rows.len());
        for row in rows {
            let mut lengths = Vec::with_capacity(columns);
            for index in 0..columns {
                let length: Option<u64> = row
                    .try_get(index)
                    .map_err(|e| DbError::driver("image length decode", e))?;
                let bytes = length.unwrap_or(0);
                total = total.checked_add(bytes).ok_or(ImageError::Budget {
                    limit: "image bytes overflow",
                })?;
                if bytes > 1_048_576 || total > 8_388_608 {
                    return Err(ImageError::Budget {
                        limit: "image field/total payload",
                    }
                    .into());
                }
                lengths.push(length);
            }
            sizes.push(lengths);
        }
        Ok(sizes)
    }
    fn decode(row: &MySqlRow, index: usize, column: &ColumnMeta) -> Result<FieldValue, DbError> {
        if row
            .try_get_raw(index)
            .map_err(|e| DbError::driver("image NULL marker", e))?
            .is_null()
        {
            return Ok(FieldValue::Null);
        }
        match column.kind() {
            ColumnType::SignedInteger => row
                .try_get(index)
                .map(FieldValue::Signed)
                .map_err(|e| DbError::driver("signed image", e)),
            ColumnType::UnsignedInteger => row
                .try_get(index)
                .map(FieldValue::Unsigned)
                .map_err(|e| DbError::driver("unsigned image", e)),
            ColumnType::Text => row
                .try_get(index)
                .map(FieldValue::Text)
                .map_err(|e| DbError::driver("text image", e)),
            ColumnType::Binary => row
                .try_get(index)
                .map(FieldValue::Binary)
                .map_err(|e| DbError::driver("binary image", e)),
            ColumnType::Decimal => {
                let value: BigDecimal = row
                    .try_get(index)
                    .map_err(|e| DbError::driver("decimal image", e))?;
                Ok(FieldValue::Decimal(DecimalValue::new(
                    &value.to_plain_string(),
                )?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::MySqlImageReader;
    use crate::DbError;
    use seata_at::SqlRecognizerFactory;
    use seata_datasource::{FieldValue, ImageError};

    #[test]
    fn oversized_parameters_are_rejected_before_native_argument_encoding() {
        let one = SqlRecognizerFactory::recognize("DELETE FROM account WHERE a=?").unwrap();
        assert!(matches!(
            MySqlImageReader::validate_request(&one, &[]),
            Err(DbError::Input { .. })
        ));
        assert!(matches!(
            MySqlImageReader::validate_request(&one, &[FieldValue::Binary(vec![0; 1_048_577])]),
            Err(DbError::Image(ImageError::Budget { .. }))
        ));
        let many =
            SqlRecognizerFactory::recognize("DELETE FROM account WHERE a IN (?,?,?,?,?,?,?,?,?)")
                .unwrap();
        let parameters = vec![FieldValue::Binary(vec![0; 1_000_000]); 9];
        assert!(matches!(
            MySqlImageReader::validate_request(&many, &parameters),
            Err(DbError::Image(ImageError::Budget { .. }))
        ));
        assert!(
            MySqlImageReader::validate_request(&one, &[FieldValue::Unsigned(u64::MAX)]).is_ok()
        );
    }
}
