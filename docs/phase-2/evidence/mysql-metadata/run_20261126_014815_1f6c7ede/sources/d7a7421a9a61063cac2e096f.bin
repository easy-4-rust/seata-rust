use crate::{exec::StatementCallback, mysql_image_reader::MySqlImageReader, DbError};
use seata_at::{SqlRecognizer, SqlType, SqlUndoLog};
use seata_datasource::FieldValue;
use sqlx::{MySql, Transaction};

/// UPDATE全列前镜像、原PK及后镜像执行器；不以更新后的WHERE重新寻找行。
/// 对应 Java: org.apache.seata.rm.datasource.exec.UpdateExecutor（当前MySQL类型子集）。
pub(crate) struct UpdateExecutor;
impl UpdateExecutor {
    /// 参数为原事务/schema/SQL/参数；返回原更新计数及内存Undo准备，尚未提交或落库。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<(u64, SqlUndoLog), DbError> {
        let before = MySqlImageReader::read(tx, database, sql, parameters).await?;
        let keys = MySqlImageReader::primary_values(&before)?;
        let count = StatementCallback::execute(tx, sql, parameters).await?;
        let after = MySqlImageReader::read_keys(tx, before.table_meta(), &keys).await?;
        Ok((count, SqlUndoLog::new(SqlType::Update, before, after)?))
    }
}
