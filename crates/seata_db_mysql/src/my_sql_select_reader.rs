use crate::{cache::MySqlTableMetaReader, mysql_image_reader::MySqlImageReader, DbError};
use seata_at::MySqlSelectRecognizer;
use seata_datasource::{ColumnType, FieldValue, TableMeta};
use sqlx::{Column, MySql, Row, Transaction, ValueRef};

/// 原事务中的有界 RR 快照读取，保留原生行顺序/类型，不执行 UDF 或发布部分结果。
/// 对应 Java: 无直接对象；Rust ORM 只读查询与原有式连接预算扩展。
pub(crate) struct MySqlSelectReader;
impl MySqlSelectReader {
    /// 参数为原拥有式事务、schema、密封读计划与原绑定；返回有界且已预检的 SQLx 原生行。
    pub(crate) async fn read(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        sql: &MySqlSelectRecognizer,
        parameters: &[FieldValue],
    ) -> Result<Vec<sqlx::mysql::MySqlRow>, DbError> {
        let table = MySqlTableMetaReader::fetch(tx, database, sql.table_name()).await?;
        let selected = if sql.is_wildcard() {
            table
                .columns()
                .iter()
                .map(|column| (column.name().to_owned(), column.name().to_owned()))
                .collect::<Vec<_>>()
        } else {
            sql.columns().to_vec()
        };
        let mut columns = Vec::new();
        let mut lengths = Vec::new();
        let mut values = Vec::new();
        let owner = TableMeta::quote_identifier(sql.qualifier())?;
        for (source, label) in &selected {
            let column = table.column(source).ok_or(DbError::Input {
                reason: "unknown read column",
            })?;
            let name = format!("{owner}.{}", TableMeta::quote_identifier(column.name())?);
            lengths.push(MySqlImageReader::length_projection(column, &name));
            values.push(format!("{name} AS {}", TableMeta::quote_identifier(label)?));
            columns.push(column);
        }
        let suffix = sql.stable_suffix_sql(&table).map_err(|_| DbError::Input {
            reason: "read metadata identity differs",
        })?;
        let preflight = format!("SELECT {}{suffix}", lengths.join(", "));
        MySqlImageReader::sql_budget(&preflight)?;
        let rows = sqlx::query_with(&preflight, MySqlImageReader::arguments(parameters)?)
            .fetch_all(&mut **tx)
            .await
            .map_err(|error| DbError::driver("read length preflight", error))?;
        let sizes = MySqlImageReader::preflight_sizes(&rows, columns.len())?;
        if sizes.is_empty() {
            return Ok(Vec::new());
        }
        let query = format!("SELECT {}{suffix}", values.join(", "));
        MySqlImageReader::sql_budget(&query)?;
        let rows = sqlx::query_with(&query, MySqlImageReader::arguments(parameters)?)
            .fetch_all(&mut **tx)
            .await
            .map_err(|error| DbError::driver("read native values", error))?;
        if rows.len() != sizes.len() {
            return Err(DbError::Input {
                reason: "snapshot read count changed",
            });
        }
        for (row, sizes) in rows.iter().zip(&sizes) {
            for (index, column) in columns.iter().enumerate() {
                if row.columns()[index].name() != selected[index].1 {
                    return Err(DbError::Input {
                        reason: "read column order differs",
                    });
                }
                let raw = row
                    .try_get_raw(index)
                    .map_err(|error| DbError::driver("read NULL marker", error))?;
                if raw.is_null() != sizes[index].is_none() {
                    return Err(DbError::Input {
                        reason: "snapshot read NULL marker changed",
                    });
                }
                if !raw.is_null()
                    && matches!(
                        column.kind(),
                        ColumnType::Text | ColumnType::Binary | ColumnType::Decimal
                    )
                {
                    let bytes = row
                        .try_get_unchecked::<&[u8], _>(index)
                        .map_err(|error| DbError::driver("read payload bytes", error))?;
                    if Some(bytes.len() as u64) != sizes[index] {
                        return Err(DbError::Input {
                            reason: "snapshot read byte count changed",
                        });
                    }
                }
            }
        }
        Ok(rows)
    }
}
