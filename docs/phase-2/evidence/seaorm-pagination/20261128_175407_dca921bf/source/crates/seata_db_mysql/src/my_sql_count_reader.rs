use crate::{cache::MySqlTableMetaReader, mysql_image_reader::MySqlImageReader, DbError};
use seata_at::MySqlCountRecognizer;
use seata_datasource::FieldValue;
use sqlx::{MySql, Row, Transaction};
/// 同一 RR 事务中的内建标量计数，不以行探测上限截断统计输入。
/// 对应 Java: 无直接对象；Rust ORM 统计读取扩展。
pub(crate) struct MySqlCountReader;
impl MySqlCountReader {
    /// 参数为原事务/schema/密封统计计划及原绑定；返回最多一个原生非空无符号计数行。
    pub(crate) async fn read(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        sql: &MySqlCountRecognizer,
        parameters: &[FieldValue],
    ) -> Result<Vec<sqlx::mysql::MySqlRow>, DbError> {
        MySqlTableMetaReader::fetch(tx, database, sql.table_name()).await?;
        MySqlImageReader::sql_budget(sql.sql())?;
        let rows = sqlx::query_with(sql.sql(), MySqlImageReader::arguments(parameters)?)
            .fetch_all(&mut **tx)
            .await
            .map_err(|error| DbError::driver("snapshot count", error))?;
        if rows.len() != 1 || rows[0].len() != 1 {
            return Err(DbError::Input {
                reason: "single count scalar required",
            });
        }
        let _: u64 = rows[0]
            .try_get(0)
            .map_err(|error| DbError::driver("unsigned count scalar", error))?;
        Ok(rows)
    }
}
