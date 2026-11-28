use crate::{sea_orm_mysql_bindings::SeaOrmMySqlBindings, SeaOrmMySqlError};
use sea_orm::{DbBackend, QueryResult, Statement};
use seata_at::MySqlSelectRecognizer;
use seata_datasource::FieldValue;
use seata_db_mysql::MySqlLocalTransaction;
/// 原事务的已验证有界 SELECT，保留原生 SQLx 行及列序，不进行 BTreeMap 结果模拟。
/// 对应 Java: 无直接对象；Rust SeaORM 查询输入适配扩展。
#[derive(Debug)]
pub(crate) struct SeaOrmMySqlReadStatement {
    recognizer: MySqlSelectRecognizer,
    parameters: Vec<FieldValue>,
}
impl SeaOrmMySqlReadStatement {
    /// 参数为拥有式原 Statement；返回 MySQL 有界只读计划，错误只含静态原因。
    pub(crate) fn from_statement(statement: Statement) -> Result<Self, SeaOrmMySqlError> {
        if statement.db_backend != DbBackend::MySql {
            return Err(SeaOrmMySqlError::Input {
                reason: "MySQL read backend required",
            });
        }
        let recognizer = MySqlSelectRecognizer::recognize(&statement.sql)?;
        let parameters = SeaOrmMySqlBindings::convert(
            statement.values.map(|v| v.0).unwrap_or_default(),
            recognizer.parameter_count(),
        )?;
        recognizer.validate_binding_bounds(&parameters)?;
        Ok(Self {
            recognizer,
            parameters,
        })
    }
    /// 参数为已拥有的原 AT 事务；返回同一快照上的原生查询行，不创建 ORM 池或提交。
    pub(crate) async fn execute(
        &self,
        transaction: &mut MySqlLocalTransaction,
    ) -> Result<Vec<QueryResult>, SeaOrmMySqlError> {
        Ok(transaction
            .query(&self.recognizer, &self.parameters)
            .await?
            .into_iter()
            .map(QueryResult::from)
            .collect())
    }
}
