use crate::{
    my_sql_count_reader::MySqlCountReader, my_sql_select_reader::MySqlSelectReader, DbError,
};
use seata_at::{MySqlCountRecognizer, MySqlSelectRecognizer, SqlError};
use seata_datasource::FieldValue;
use sqlx::{MySql, Transaction};
/// 数据读取与计数共用同一事务准入/取消/失败边界的内部策略。
/// 对应 Java: 无直接对象；Rust 拥有式读取策略扩展。
pub(crate) enum MySqlReadOperation<'plan> {
    Select(&'plan MySqlSelectRecognizer),
    Count(&'plan MySqlCountRecognizer),
}
impl MySqlReadOperation<'_> {
    /// 无参数；返回同一真实资源表。
    pub(crate) fn table_name(&self) -> &str {
        match self {
            Self::Select(plan) => plan.table_name(),
            Self::Count(plan) => plan.table_name(),
        }
    }
    /// 参数为原绑定；返回同一形状与预算校验结果。
    pub(crate) fn validate_binding_bounds(
        &self,
        parameters: &[FieldValue],
    ) -> Result<(), SqlError> {
        match self {
            Self::Select(plan) => plan.validate_binding_bounds(parameters),
            Self::Count(plan) => plan.validate_binding_bounds(parameters),
        }
    }
    /// 参数为原事务/schema/绑定；返回原生有界结果，不执行独立事务控制。
    pub(crate) async fn execute(
        &self,
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        parameters: &[FieldValue],
    ) -> Result<Vec<sqlx::mysql::MySqlRow>, DbError> {
        match self {
            Self::Select(plan) => MySqlSelectReader::read(tx, database, plan, parameters).await,
            Self::Count(plan) => MySqlCountReader::read(tx, database, plan, parameters).await,
        }
    }
}
