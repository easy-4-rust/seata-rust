use crate::{
    cache::MySqlTableMetaReader, exec::StatementCallback, mysql_image_reader::MySqlImageReader,
    DbError,
};
use seata_at::{SqlRecognizer, SqlType, SqlUndoLog};
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, Field, FieldValue, ImageError, TableMeta, TableRecords,
};
use sqlx::{MySql, Row, Transaction};

/// 显式完整PK的INSERT执行器，原表达式/原参数确定主键，不猜自增序列。
/// 对应 Java: org.apache.seata.rm.datasource.exec.mysql.MySQLInsertExecutor#getPkValuesByColumn（部分实现）。
pub(crate) struct MySqlInsertExecutor;
impl MySqlInsertExecutor {
    /// 参数为原事务/schema/正式SQL/参数；返回原计数及空before/完整after的内存Undo准备。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<(u64, SqlUndoLog), DbError> {
        let table = MySqlTableMetaReader::fetch(tx, database, sql.table_name()).await?;
        let keys = Self::primary_values(tx, &table, sql, parameters).await?;
        let count = StatementCallback::execute(tx, sql, parameters).await?;
        let after = MySqlImageReader::read_keys(tx, &table, &keys).await?;
        if count != sql.insert_row_count() as u64 || after.rows().len() != sql.insert_row_count() {
            return Err(DbError::Input {
                reason: "INSERT count and complete primary image differ",
            });
        }
        Ok((
            count,
            SqlUndoLog::new(SqlType::Insert, TableRecords::new(table), after)?,
        ))
    }
    async fn primary_values(
        tx: &mut Transaction<'static, MySql>,
        table: &TableMeta,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<Vec<Vec<FieldValue>>, DbError> {
        for name in sql.columns() {
            if table.column(name).is_none() {
                return Err(DbError::Input {
                    reason: "unknown INSERT column",
                });
            }
        }
        let primary: Vec<_> = table.primary_key_columns().collect();
        if sql.insert_rows().len() != sql.insert_row_count()
            || sql
                .insert_rows()
                .len()
                .checked_mul(primary.len())
                .is_none_or(|cells| cells > 8192)
        {
            return Err(ImageError::Budget {
                limit: "INSERT key cells",
            }
            .into());
        }
        let positions = primary
            .iter()
            .map(|column| {
                sql.columns()
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case(column.name()))
                    .ok_or(DbError::Unsupported {
                        feature: "all INSERT primary columns must be explicit",
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut selects = Vec::with_capacity(sql.insert_rows().len());
        let mut bindings = Vec::new();
        for (index, row) in sql.insert_rows().iter().enumerate() {
            let mut fields = Vec::with_capacity(primary.len() + 1);
            for (key_index, (column, position)) in primary.iter().zip(&positions).enumerate() {
                let value = row.get(*position).ok_or(DbError::Input {
                    reason: "complete INSERT value row required",
                })?;
                let end = value
                    .parameter_start()
                    .checked_add(value.parameter_count())
                    .ok_or(DbError::Input {
                        reason: "INSERT parameter range overflow",
                    })?;
                let args = parameters
                    .get(value.parameter_start()..end)
                    .ok_or(DbError::Input {
                        reason: "INSERT parameter range",
                    })?;
                bindings.extend_from_slice(args);
                fields.push(format!(
                    "{} AS `k{key_index}`",
                    Self::cast(column, value.sql())?
                ));
            }
            fields.push(format!("CAST({index} AS UNSIGNED) AS `__seata_row`"));
            selects.push(format!("SELECT {}", fields.join(", ")));
        }
        let query = format!("{} ORDER BY `__seata_row`", selects.join(" UNION ALL "));
        MySqlImageReader::sql_budget(&query)?;
        let rows = sqlx::query_with(&query, MySqlImageReader::arguments(&bindings)?)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| DbError::driver("INSERT primary expressions", e))?;
        if rows.len() != sql.insert_row_count() {
            return Err(DbError::Input {
                reason: "INSERT primary expression rows",
            });
        }
        let mut keys = Vec::with_capacity(rows.len());
        let mut bytes = 0usize;
        for (index, row) in rows.iter().enumerate() {
            let ordinal: u64 = row
                .try_get(primary.len())
                .map_err(|e| DbError::driver("INSERT row ordinal", e))?;
            if ordinal != index as u64 {
                return Err(DbError::Input {
                    reason: "INSERT expression order",
                });
            }
            let mut key = Vec::with_capacity(primary.len());
            for (position, column) in primary.iter().enumerate() {
                let value = MySqlImageReader::decode(row, position, column)?;
                bytes = bytes
                    .checked_add(value.value_bytes())
                    .ok_or(ImageError::Budget {
                        limit: "INSERT key bytes overflow",
                    })?;
                if bytes > 8_388_608 {
                    return Err(ImageError::Budget {
                        limit: "INSERT key payload",
                    }
                    .into());
                }
                Field::new(column, value.clone())?;
                key.push(value);
            }
            keys.push(key);
        }
        Ok(keys)
    }
    fn cast(column: &ColumnMeta, expression: &str) -> Result<String, DbError> {
        match column.domain().ok_or(DbError::Unsupported {
            feature: "physical primary column domain",
        })? {
            ColumnDomain::Integer { unsigned, .. } => Ok(format!(
                "CAST(({expression}) AS {})",
                if *unsigned { "UNSIGNED" } else { "SIGNED" }
            )),
            ColumnDomain::Decimal { .. } => Ok(format!(
                "CAST(({expression}) AS DECIMAL({},{}))",
                column.numeric_precision().ok_or(DbError::Input {
                    reason: "primary precision"
                })?,
                column.numeric_scale().ok_or(DbError::Input {
                    reason: "primary scale"
                })?
            )),
            ColumnDomain::Text {
                charset,
                character_limit,
                fixed_width,
            } => {
                let encoding = match charset {
                    CharacterSet::Ascii => "ascii",
                    CharacterSet::Utf8Mb3 => "utf8mb3",
                    CharacterSet::Utf8Mb4 => "utf8mb4",
                };
                let length = character_limit.map_or_else(String::new, |n| format!("({n})"));
                let cast = format!("CAST(({expression}) AS CHAR{length} CHARACTER SET {encoding})");
                Ok(if *fixed_width {
                    format!("RTRIM({cast})")
                } else {
                    cast
                })
            }
            ColumnDomain::Binary { fixed_width } => {
                let length = if *fixed_width {
                    format!(
                        "({})",
                        column.char_octet_length().ok_or(DbError::Input {
                            reason: "primary binary length"
                        })?
                    )
                } else {
                    String::new()
                };
                Ok(format!("CAST(({expression}) AS BINARY{length})"))
            }
        }
    }
}
