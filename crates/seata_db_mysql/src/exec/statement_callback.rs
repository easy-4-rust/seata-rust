use crate::{mysql_image_reader::MySqlImageReader, DbError, MySqlStatementResult};
use seata_at::SqlRecognizer;
use seata_datasource::FieldValue;
use sqlx::{MySql, Transaction};

/// 原SQL只执行一次的原生回调，不执行重放或提交。
/// 对应 Java: org.apache.seata.rm.datasource.exec.StatementCallback的MySQL具体调用路径。
pub(crate) struct StatementCallback;
impl StatementCallback {
    /// 参数为原事务、正式SQL及完整原参数；返回同一业务语句的原始计数与生成 ID，错误只保留静态阶段/编号。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<MySqlStatementResult, DbError> {
        MySqlImageReader::validate_parameters(sql, parameters)?;
        let result = sqlx::query_with(sql.sql(), MySqlImageReader::arguments(parameters)?)
            .execute(&mut **tx)
            .await
            .map_err(|error| DbError::driver("business statement", error))?;
        Ok(MySqlStatementResult::new(
            result.rows_affected(),
            result.last_insert_id(),
        ))
    }
}
