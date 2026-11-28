use crate::SeaOrmMySqlError;
use sea_orm::QueryResult;
use seata_at::{MySqlCountRecognizer, MySqlSelectRecognizer, SqlError};
use seata_datasource::FieldValue;
use seata_db_mysql::MySqlLocalTransaction;
/// 原生模型读取与内建统计的密封分派，不提供任意函数或事务 SQL 执行入口。
/// 对应 Java: 无直接对象；Rust ORM 读取计划扩展。
#[derive(Debug)]
pub(crate) enum SeaOrmMySqlReadPlan {
    Select(MySqlSelectRecognizer),
    Count(MySqlCountRecognizer),
}
impl SeaOrmMySqlReadPlan {
    /// 参数为原 SELECT；返回只读或内建统计计划，双路径均需完整词法/AST 验证。
    pub(crate) fn recognize(sql: &str) -> Result<Self, SqlError> {
        match MySqlSelectRecognizer::recognize(sql) {
            Ok(plan) => Ok(Self::Select(plan)),
            Err(_) => MySqlCountRecognizer::recognize(sql).map(Self::Count),
        }
    }
    /// 无参数；返回完整原占位符数量。
    pub(crate) fn parameter_count(&self) -> usize {
        match self {
            Self::Select(plan) => plan.parameter_count(),
            Self::Count(plan) => plan.parameter_count(),
        }
    }
    /// 参数为完整原绑定；返回同一分页/参数预算校验。
    pub(crate) fn validate_binding_bounds(&self, values: &[FieldValue]) -> Result<(), SqlError> {
        match self {
            Self::Select(plan) => plan.validate_binding_bounds(values),
            Self::Count(plan) => plan.validate_binding_bounds(values),
        }
    }
    /// 参数为原事务与绑定；返回原生 SQLx 行转换后的 SeaORM 结果，不另建业务连接。
    pub(crate) async fn execute(
        &self,
        transaction: &mut MySqlLocalTransaction,
        values: &[FieldValue],
    ) -> Result<Vec<QueryResult>, SeaOrmMySqlError> {
        let rows = match self {
            Self::Select(plan) => transaction.query(plan, values).await?,
            Self::Count(plan) => transaction.query_count(plan, values).await?,
        };
        Ok(rows.into_iter().map(QueryResult::from).collect())
    }
}
