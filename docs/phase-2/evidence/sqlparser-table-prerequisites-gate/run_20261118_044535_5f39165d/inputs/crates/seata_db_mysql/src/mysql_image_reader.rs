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
        Self::validate_parameters(sql, parameters)
    }
    /// 参数为已验证SQL和完整原绑定；返回单元/总量预算检查，供只执行一次的DML共用。
    pub(crate) fn validate_parameters(
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<(), DbError> {
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
        Self::read_locked(tx, &table, sql.predicate_sql(), predicate_parameters).await
    }
    /// 参数为同事务MDL下的完整表、受控条件与原生绑定；返回经过长度预算预检的全列镜像。
    pub(crate) async fn read_locked(
        tx: &mut Transaction<'static, MySql>,
        table: &TableMeta,
        predicate: Option<&str>,
        parameters: &[FieldValue],
    ) -> Result<TableRecords, DbError> {
        let suffix = Self::suffix(table, predicate)?;
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
        let preflight_sql = format!("SELECT {lengths}{suffix}");
        Self::sql_budget(&preflight_sql)?;
        let preflight = sqlx::query_with(&preflight_sql, Self::arguments(parameters)?)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| DbError::driver("image length preflight", e))?;
        let sizes = Self::preflight_sizes(&preflight, table.columns().len())?;
        let mut records = TableRecords::new(table.clone());
        if sizes.is_empty() {
            return Ok(records);
        }
        let values_sql = format!("SELECT {}{suffix}", quoted.join(", "));
        Self::sql_budget(&values_sql)?;
        let values = sqlx::query_with(&values_sql, Self::arguments(parameters)?)
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
            records.add(ImageRow::new(table, fields)?)?;
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
    fn suffix(table: &TableMeta, predicate: Option<&str>) -> Result<String, DbError> {
        if predicate.is_some_and(|value| value.len() > 1_048_576) {
            return Err(ImageError::Budget {
                limit: "generated SQL",
            }
            .into());
        }
        let predicate = predicate.map_or_else(String::new, |p| format!(" WHERE {p}"));
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
    /// 参数为有限原生值；返回SQLx编码参数，不把DECIMAL降为字符串类型。
    pub(crate) fn arguments(parameters: &[FieldValue]) -> Result<MySqlArguments, DbError> {
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
    /// 参数为原数据库行/位置及真实列；返回精确且保留NULL的值，未复制服务器错误文本。
    pub(crate) fn decode(
        row: &MySqlRow,
        index: usize,
        column: &ColumnMeta,
    ) -> Result<FieldValue, DbError> {
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
    /// 参数为内部生成SQL；超过1MiB时在发送前明确拒绝。
    pub(crate) fn sql_budget(sql: &str) -> Result<(), DbError> {
        if sql.len() > 1_048_576 {
            Err(ImageError::Budget {
                limit: "1MiB generated SQL",
            }
            .into())
        } else {
            Ok(())
        }
    }
    /// 参数为完整镜像；返回索引顺序的原PK值，每个字段保持精确类型。
    pub(crate) fn primary_values(records: &TableRecords) -> Result<Vec<Vec<FieldValue>>, DbError> {
        let keys: Vec<Vec<_>> = records
            .rows()
            .iter()
            .map(|row| {
                row.primary_keys()
                    .map(|field| field.value().clone())
                    .collect()
            })
            .collect();
        Self::validate_keys(records.table_meta(), &keys)?;
        Ok(keys)
    }
    /// 参数为真实表与原PK元组；在一次全序查询内取镜像，不复用可能变化的业务WHERE。
    pub(crate) async fn read_keys(
        tx: &mut Transaction<'static, MySql>,
        table: &TableMeta,
        keys: &[Vec<FieldValue>],
    ) -> Result<TableRecords, DbError> {
        Self::validate_keys(table, keys)?;
        if keys.is_empty() {
            return Ok(TableRecords::new(table.clone()));
        }
        let names = table
            .primary_key_columns()
            .map(|c| TableMeta::quote_identifier(c.name()))
            .collect::<Result<Vec<_>, _>>()?;
        let tuple = format!(
            "({})",
            names
                .iter()
                .map(|name| format!("{name} = ?"))
                .collect::<Vec<_>>()
                .join(" AND ")
        );
        let predicate = vec![tuple; keys.len()].join(" OR ");
        let values: Vec<_> = keys.iter().flat_map(|key| key.iter().cloned()).collect();
        Self::read_locked(tx, table, Some(&predicate), &values).await
    }
    fn validate_keys(table: &TableMeta, keys: &[Vec<FieldValue>]) -> Result<(), DbError> {
        let columns: Vec<_> = table.primary_key_columns().collect();
        if keys.len() > 1024
            || keys
                .len()
                .checked_mul(columns.len())
                .is_none_or(|cells| cells > 8192)
        {
            return Err(ImageError::Budget {
                limit: "1024 key rows/8192 key cells",
            }
            .into());
        }
        let mut total = 0usize;
        for key in keys {
            if key.len() != columns.len() {
                return Err(DbError::Input {
                    reason: "complete primary tuple required",
                });
            }
            for (value, column) in key.iter().zip(&columns) {
                total = total
                    .checked_add(value.value_bytes())
                    .ok_or(ImageError::Budget {
                        limit: "key payload overflow",
                    })?;
                if value.value_bytes() > 1_048_576 || total > 8_388_608 {
                    return Err(ImageError::Budget {
                        limit: "key payload",
                    }
                    .into());
                }
                Field::new(column, value.clone())?;
            }
        }
        Ok(())
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
