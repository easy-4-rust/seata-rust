use crate::RbatisMySqlError;
use seata_at::{MySqlCountRecognizer, MySqlSelectRecognizer, SqlError};
use seata_datasource::FieldValue;
use seata_db_mysql::MySqlLocalTransaction;
use sqlx::mysql::MySqlRow;
/// 原单表 SELECT/COUNT 密封计划的分派，不接收任意函数或事务 SQL。
/// 对应 Java: 无直接对象；Rust ORM 快照读取扩展。
pub(crate) enum RbatisMySqlReadPlan {
    Select(MySqlSelectRecognizer),
    Count(MySqlCountRecognizer),
}
impl RbatisMySqlReadPlan {
    /// 参数为完整原 SQL；返回复用核心准入的普通读取或计数计划。
    pub(crate) fn recognize(sql: &str) -> Result<Self, SqlError> {
        match MySqlSelectRecognizer::recognize(sql) {
            Ok(plan) => Ok(Self::Select(plan)),
            Err(_) => MySqlCountRecognizer::recognize(sql).map(Self::Count),
        }
    }
    /// 无参数；返回原占位符数量。
    pub(crate) fn parameter_count(&self) -> usize {
        match self {
            Self::Select(p) => p.parameter_count(),
            Self::Count(p) => p.parameter_count(),
        }
    }
    /// 参数为有界原绑定；返回核心分页/偏移检查结果。
    pub(crate) fn validate(&self, values: &[FieldValue]) -> Result<(), SqlError> {
        match self {
            Self::Select(p) => p.validate_binding_bounds(values),
            Self::Count(p) => p.validate_binding_bounds(values),
        }
    }
    /// 参数为同一原事务和绑定；返回同一 RR 快照的原生 SQLx 行。
    pub(crate) async fn execute(
        &self,
        transaction: &mut MySqlLocalTransaction,
        values: &[FieldValue],
    ) -> Result<Vec<MySqlRow>, RbatisMySqlError> {
        Ok(match self {
            Self::Select(p) => transaction.query(p, values).await?,
            Self::Count(p) => transaction.query_count(p, values).await?,
        })
    }
}
