use crate::{mysql_image_reader::MySqlImageReader, DbError};
use seata_at::SqlUndoLog;
use seata_datasource::{FieldValue, Row, TableMeta, TableRecords};
use sqlx::{MySql, Transaction};
/// 删除原INSERT后镜像；使用原生值和完整PK元组，不提交。
/// 对应 Java: org.apache.seata.rm.datasource.undo.mysql.MySQLUndoInsertExecutor（既有类型范围）。
pub(crate) struct MySqlUndoInsertExecutor;
impl MySqlUndoInsertExecutor {
    /// 参数为原日志；返回原after镜像引用，保持原对象身份，不按日志 SQL 类别改变执行器选择。
    /// 对应 Java: MySQLUndoInsertExecutor#getUndoRows。
    pub(crate) fn get_undo_rows(log: &SqlUndoLog) -> &TableRecords {
        log.after_image()
    }

    /// 参数为已验证表元数据；返回实际补偿使用的参数化 SQL。
    /// 对应 Java: MySQLUndoInsertExecutor#buildUndoSQL（已认证原生类型范围）。
    pub(crate) fn build_undo_sql(table: &TableMeta) -> Result<String, DbError> {
        let predicate = table
            .primary_key_columns()
            .map(|c| Ok(format!("{} = ?", TableMeta::quote_identifier(c.name())?)))
            .collect::<Result<Vec<_>, seata_datasource::MetadataError>>()?
            .join(" AND ");
        Ok(format!(
            "DELETE FROM {} WHERE {}",
            TableMeta::quote_identifier(table.name())?,
            predicate
        ))
    }

    /// 参数为原事务、当前完整元数据及原Undo行；返回驱动更新计数，不重放原业务。
    /// 对应 Java: MySQLUndoInsertExecutor#buildUndoSQL及AbstractUndoExecutor#undoPrepare。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        table: &TableMeta,
        row: &Row,
    ) -> Result<u64, DbError> {
        let sql = Self::build_undo_sql(table)?;
        let parameters: Vec<FieldValue> = row.primary_keys().map(|f| f.value().clone()).collect();
        MySqlImageReader::sql_budget(&sql)?;
        let result = sqlx::query_with(&sql, MySqlImageReader::arguments(&parameters)?)
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("Undo insert compensation", e))?;
        Ok(result.rows_affected())
    }
}
