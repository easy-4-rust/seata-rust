use crate::{mysql_image_reader::MySqlImageReader, DbError};
use seata_at::SqlUndoLog;
use seata_datasource::{FieldValue, Row, TableMeta, TableRecords};
use sqlx::{MySql, Transaction};
/// 重新插入原DELETE前镜像全部字段；使用原生值和完整PK元组，不提交。
/// 对应 Java: org.apache.seata.rm.datasource.undo.mysql.MySQLUndoDeleteExecutor（既有类型范围）。
pub(crate) struct MySqlUndoDeleteExecutor;
impl MySqlUndoDeleteExecutor {
    /// 参数为原日志；返回原before镜像引用，保持原对象身份，不按日志 SQL 类别改变执行器选择。
    /// 对应 Java: MySQLUndoDeleteExecutor#getUndoRows。
    pub(crate) fn get_undo_rows(log: &SqlUndoLog) -> &TableRecords {
        log.before_image()
    }

    /// 参数为已验证表元数据及原行；返回实际补偿使用的参数化 SQL。
    /// 对应 Java: MySQLUndoDeleteExecutor#buildUndoSQL（已认证原生类型范围）。
    pub(crate) fn build_undo_sql(table: &TableMeta, row: &Row) -> Result<String, DbError> {
        let fields: Vec<_> = row.non_primary_keys().chain(row.primary_keys()).collect();
        let columns = fields
            .iter()
            .map(|f| TableMeta::quote_identifier(f.name()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!(
            "INSERT INTO {} ({}) VALUES ({})",
            TableMeta::quote_identifier(table.name())?,
            columns.join(", "),
            vec!["?"; columns.len()].join(", ")
        ))
    }

    /// 参数为原事务、当前完整元数据及原Undo行；返回驱动更新计数，不重放原业务。
    /// 对应 Java: MySQLUndoDeleteExecutor#buildUndoSQL及AbstractUndoExecutor#undoPrepare。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        table: &TableMeta,
        row: &Row,
    ) -> Result<u64, DbError> {
        let fields: Vec<_> = row.non_primary_keys().chain(row.primary_keys()).collect();
        let sql = Self::build_undo_sql(table, row)?;
        let parameters: Vec<FieldValue> = fields.iter().map(|f| f.value().clone()).collect();
        MySqlImageReader::sql_budget(&sql)?;
        // 显式零必须恢复原零；默认 MySQL 模式会把它再次解释为申请自增 ID。
        // 调用方拥有 close_on_drop 的补偿租约，模式仅服务于原镜像恢复，不进入业务连接复用。
        if row.primary_keys().any(|field| {
            table
                .column(field.name())
                .is_some_and(|column| column.is_auto_increment())
                && matches!(
                    field.value(),
                    FieldValue::Signed(0) | FieldValue::Unsigned(0)
                )
        }) {
            sqlx::query("SET SESSION sql_mode=IF(FIND_IN_SET('NO_AUTO_VALUE_ON_ZERO',@@SESSION.sql_mode)>0,@@SESSION.sql_mode,CONCAT_WS(',',@@SESSION.sql_mode,'NO_AUTO_VALUE_ON_ZERO'))")
                .execute(&mut **tx).await.map_err(|error|DbError::driver("preserve original zero primary compensation",error))?;
        }
        let result = sqlx::query_with(&sql, MySqlImageReader::arguments(&parameters)?)
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("Undo delete compensation", e))?;
        Ok(result.rows_affected())
    }
}
