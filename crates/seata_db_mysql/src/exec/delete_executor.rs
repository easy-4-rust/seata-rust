use crate::{
    exec::StatementCallback, mysql_image_reader::MySqlImageReader, DbError, MySqlStatementResult,
};
use seata_at::{SqlRecognizer, SqlType, SqlUndoLog};
use seata_datasource::FieldValue;
use sqlx::{MySql, Transaction};

/// DELETE保留原全列镜像并核对原PK已不存在，失败不能发布部分Undo。
/// 对应 Java: org.apache.seata.rm.datasource.exec.DeleteExecutor（当前MySQL类型子集）。
pub(crate) struct DeleteExecutor;
impl DeleteExecutor {
    /// 参数为原事务/schema/SQL/参数；返回原更新计数及内存Undo准备，不提交事务。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<(MySqlStatementResult, SqlUndoLog), DbError> {
        let before = MySqlImageReader::read(tx, database, sql, parameters).await?;
        let keys = MySqlImageReader::primary_values(&before)?;
        let result = StatementCallback::execute(tx, sql, parameters).await?;
        let after = MySqlImageReader::read_keys(tx, before.table_meta(), &keys).await?;
        Ok((result, SqlUndoLog::new(SqlType::Delete, before, after)?))
    }
}
