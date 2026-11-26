use crate::{mysql_image_reader::MySqlImageReader, DbError};
use seata_datasource::{FieldValue, Row, TableMeta};
use sqlx::{MySql, Transaction};
/// 恢复原UPDATE前镜像的非PK字段；使用原生值和完整PK元组，不提交。
/// 对应 Java: org.apache.seata.rm.datasource.undo.mysql.MySQLUndoUpdateExecutor（既有类型范围）。
pub(crate) struct MySqlUndoUpdateExecutor;
impl MySqlUndoUpdateExecutor {
    /// 参数为原事务、当前完整元数据及原Undo行；返回驱动更新计数，不重放原业务。
    /// 对应 Java: MySQLUndoUpdateExecutor#buildUndoSQL及AbstractUndoExecutor#undoPrepare。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        table: &TableMeta,
        row: &Row,
    ) -> Result<u64, DbError> {
        let predicate = table
            .primary_key_columns()
            .map(|c| Ok(format!("{} = ?", TableMeta::quote_identifier(c.name())?)))
            .collect::<Result<Vec<_>, seata_datasource::MetadataError>>()?
            .join(" AND ");
        let columns = row
            .non_primary_keys()
            .map(|f| Ok(format!("{} = ?", TableMeta::quote_identifier(f.name())?)))
            .collect::<Result<Vec<_>, seata_datasource::MetadataError>>()?;
        if columns.is_empty() {
            return Err(DbError::Unsupported {
                feature: "Undo UPDATE without non-PK columns",
            });
        }
        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            TableMeta::quote_identifier(table.name())?,
            columns.join(", "),
            predicate
        );
        let parameters: Vec<FieldValue> = row
            .non_primary_keys()
            .chain(row.primary_keys())
            .map(|f| f.value().clone())
            .collect();
        MySqlImageReader::sql_budget(&sql)?;
        let result = sqlx::query_with(&sql, MySqlImageReader::arguments(&parameters)?)
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("Undo update compensation", e))?;
        Ok(result.rows_affected())
    }
}
