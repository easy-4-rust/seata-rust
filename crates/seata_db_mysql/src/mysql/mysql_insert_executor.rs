use crate::{
    cache::MySqlTableMetaReader, exec::StatementCallback, mysql_image_reader::MySqlImageReader,
    DbError,
};
use seata_at::{SqlRecognizer, SqlType, SqlUndoLog};
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, Field, FieldValue, ImageError, TableMeta, TableRecords,
};
use sqlx::{MySql, Row, Transaction};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_STAGING_TABLE: AtomicU64 = AtomicU64::new(1);

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
        let temp = Self::temporary_name(table)?;
        let mut definitions = vec!["`r` BIGINT UNSIGNED NOT NULL PRIMARY KEY".to_owned()];
        for (index, column) in primary.iter().enumerate() {
            definitions.push(format!("`k{index}` {} NOT NULL", Self::definition(column)?));
        }
        let create = format!(
            "CREATE TEMPORARY TABLE {temp} ({}) ENGINE=InnoDB",
            definitions.join(", ")
        );
        MySqlImageReader::sql_budget(&create)?;
        // 必须走真实列赋值，而不是CAST；后者对小数字符串可能截断成相邻旧主键。
        sqlx::query(&create)
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("create INSERT key staging", e))?;
        let mut rows = Vec::with_capacity(sql.insert_rows().len());
        let mut bindings = Vec::new();
        for (index, row) in sql.insert_rows().iter().enumerate() {
            let mut values = vec![index.to_string()];
            for position in &positions {
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
                values.push(format!("({})", value.sql()));
            }
            rows.push(format!("({})", values.join(", ")));
        }
        let insert = format!("INSERT INTO {temp} VALUES {}", rows.join(", "));
        MySqlImageReader::sql_budget(&insert)?;
        let staged = sqlx::query_with(&insert, MySqlImageReader::arguments(&bindings)?)
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("assign INSERT key staging", e))?;
        if staged.rows_affected() != sql.insert_row_count() as u64 {
            return Err(DbError::Input {
                reason: "INSERT key assignment count",
            });
        }
        let mut columns = (0..primary.len())
            .map(|index| MySqlImageReader::projection(primary[index], &format!("`k{index}`")))
            .collect::<Vec<_>>();
        columns.push("`r`".to_owned());
        let select = format!(
            "SELECT {} FROM {temp} ORDER BY `r` LIMIT 1025",
            columns.join(", ")
        );
        let rows = sqlx::query(&select)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| DbError::driver("read INSERT key staging", e))?;
        if rows.len() != sql.insert_row_count() {
            return Err(DbError::Input {
                reason: "INSERT primary assignment rows",
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
        // 仅CREATE/DROP TEMPORARY，绝不ALTER/CREATE普通表；失败时poison并由拥有式连接退出清理。
        sqlx::query(&format!("DROP TEMPORARY TABLE {temp}"))
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("drop INSERT key staging", e))?;
        Ok(keys)
    }
    fn temporary_name(table: &TableMeta) -> Result<String, DbError> {
        for _ in 0..2 {
            let sequence = NEXT_STAGING_TABLE.fetch_add(1, Ordering::Relaxed);
            let name = format!("__seata_at_keys_{sequence:x}");
            if !name.eq_ignore_ascii_case(table.name()) {
                return Ok(format!("`{name}`"));
            }
        }
        Err(DbError::Input {
            reason: "temporary key identifier allocation",
        })
    }
    fn definition(column: &ColumnMeta) -> Result<String, DbError> {
        match column.domain().ok_or(DbError::Unsupported {
            feature: "physical primary column domain",
        })? {
            ColumnDomain::LocalDateTime { fractional_seconds } => {
                Ok(format!("DATETIME({fractional_seconds})"))
            }
            ColumnDomain::Json => Err(DbError::Unsupported {
                feature: "JSON primary key",
            }),
            ColumnDomain::Integer { bits, unsigned } => {
                let kind = match bits {
                    8 => "TINYINT",
                    16 => "SMALLINT",
                    24 => "MEDIUMINT",
                    32 => "INT",
                    64 => "BIGINT",
                    _ => {
                        return Err(DbError::Unsupported {
                            feature: "physical primary width",
                        })
                    }
                };
                Ok(format!(
                    "{kind}{}",
                    if *unsigned { " UNSIGNED" } else { "" }
                ))
            }
            ColumnDomain::Decimal { unsigned } => Ok(format!(
                "DECIMAL({},{}){}",
                column.numeric_precision().ok_or(DbError::Input {
                    reason: "primary precision"
                })?,
                column.numeric_scale().ok_or(DbError::Input {
                    reason: "primary scale"
                })?,
                if *unsigned { " UNSIGNED" } else { "" }
            )),
            ColumnDomain::Text {
                charset,
                character_limit,
                fixed_width,
            } => {
                let length = character_limit.ok_or(DbError::Unsupported {
                    feature: "unprefixed text LOB primary key",
                })?;
                let encoding = match charset {
                    CharacterSet::Ascii => "ascii",
                    CharacterSet::Utf8Mb3 => "utf8mb3",
                    CharacterSet::Utf8Mb4 => "utf8mb4",
                };
                Ok(format!(
                    "{}({length}) CHARACTER SET {encoding}",
                    if *fixed_width { "CHAR" } else { "VARCHAR" }
                ))
            }
            ColumnDomain::Binary { fixed_width } => Ok(format!(
                "{}({})",
                if *fixed_width { "BINARY" } else { "VARBINARY" },
                column.char_octet_length().ok_or(DbError::Input {
                    reason: "primary binary length"
                })?
            )),
        }
    }
}
