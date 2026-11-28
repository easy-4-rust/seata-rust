use crate::{sea_orm_mysql_read_statement::SeaOrmMySqlReadStatement, SeaOrmMySqlStatement};
use sea_orm::{
    ConnectionTrait, DbBackend, DbErr, ExecResult, ProxyExecResult, QueryResult, Statement,
};
use seata_db_mysql::MySqlLocalTransaction;
use std::fmt;
use tokio::sync::Mutex;

/// 独占借用原 AT 事务的原生 SeaORM 连接；释放借用后才可交回正式准备/提交入口。
/// 不实现 TransactionTrait，不发布连接、池或绕过 Undo 的提交能力。
/// 对应 Java: 无直接对象；Rust ORM 的 ConnectionProxy 借用式执行扩展。
pub struct SeaOrmMySqlConnection<'transaction> {
    transaction: Mutex<&'transaction mut MySqlLocalTransaction>,
}
impl<'transaction> SeaOrmMySqlConnection<'transaction> {
    /// 参数为已拥有的本地 AT 事务；返回只在该可变借用范围内有效的 ORM 连接。
    /// 释放本对象后，调用方继续原 prepare/commit/rollback；本对象没有独立事务入口。
    /// ```compile_fail
    /// fn forbidden(connection: &seata_orm_seaorm::SeaOrmMySqlConnection<'_>) {
    ///     let _ = sea_orm::TransactionTrait::begin(connection);
    /// }
    /// ```
    pub fn new(transaction: &'transaction mut MySqlLocalTransaction) -> Self {
        Self {
            transaction: Mutex::new(transaction),
        }
    }
}
#[async_trait::async_trait]
impl ConnectionTrait for SeaOrmMySqlConnection<'_> {
    fn get_database_backend(&self) -> DbBackend {
        DbBackend::MySql
    }
    async fn execute(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        let plan = SeaOrmMySqlStatement::from_statement(statement)
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        let mut transaction = self.transaction.lock().await;
        let result = plan
            .execute_with_result(&mut transaction)
            .await
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        Ok(ProxyExecResult::new(result.last_insert_id(), result.rows_affected()).into())
    }
    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        self.execute(Statement::from_string(DbBackend::MySql, sql.to_owned()))
            .await
    }
    async fn query_one(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        Ok(self.query_all(statement).await?.into_iter().next())
    }
    async fn query_all(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        let plan = SeaOrmMySqlReadStatement::from_statement(statement)
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        let mut transaction = self.transaction.lock().await;
        plan.execute(&mut transaction)
            .await
            .map_err(|error| DbErr::Custom(error.to_string()))
    }
}
impl fmt::Debug for SeaOrmMySqlConnection<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("SeaOrmMySqlConnection")
            .finish_non_exhaustive()
    }
}
