use crate::{mysql_image_reader::MySqlImageReader, DbError};
use seata_datasource::{FieldValue, Row, TableMeta};
use sqlx::{MySql, Transaction};
/// 重新插入原DELETE前镜像全部字段；使用原生值和完整PK元组，不提交。
/// 对应 Java: org.apache.seata.rm.datasource.undo.mysql.MySQLUndoDeleteExecutor（既有类型范围）。
pub(crate) struct MySqlUndoDeleteExecutor;
impl MySqlUndoDeleteExecutor {
    /// 参数为原事务、当前完整元数据及原Undo行；返回驱动更新计数，不重放原业务。
    /// 对应 Java: MySQLUndoDeleteExecutor#buildUndoSQL及AbstractUndoExecutor#undoPrepare。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        table: &TableMeta,
        row: &Row,
    ) -> Result<u64, DbError> {
        let fields: Vec<_> = row.non_primary_keys().chain(row.primary_keys()).collect();
        let columns = fields
            .iter()
            .map(|f| TableMeta::quote_identifier(f.name()))
            .collect::<Result<Vec<_>, _>>()?;
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            TableMeta::quote_identifier(table.name())?,
            columns.join(", "),
            vec!["?"; columns.len()].join(", ")
        );
        let parameters: Vec<FieldValue> = fields.iter().map(|f| f.value().clone()).collect();
        MySqlImageReader::sql_budget(&sql)?;
        let result = sqlx::query_with(&sql, MySqlImageReader::arguments(&parameters)?)
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("Undo delete compensation", e))?;
        Ok(result.rows_affected())
    }
}
