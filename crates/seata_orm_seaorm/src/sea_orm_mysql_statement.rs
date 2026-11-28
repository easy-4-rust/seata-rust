use crate::SeaOrmMySqlError;
use sea_orm::{DbBackend, QueryTrait, Statement};
use seata_at::{SqlRecognizer, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use seata_db_mysql::MySqlLocalTransaction;
use std::fmt;

/// 已完整验证且保持原 SQL/有序绑定的 SeaORM MySQL DML；本身不拥有或提交连接。
/// 对应 Java: 无直接对象；PreparedStatementProxy 的 Rust ORM 输入适配扩展。
pub struct SeaOrmMySqlStatement {
    recognizer: SqlRecognizer,
    parameters: Vec<FieldValue>,
}
impl SeaOrmMySqlStatement {
    /// 参数为真实 SeaORM 查询构建器；返回 MySQL 方言的完整已验证 DML，不内插绑定值。
    pub fn from_query<Q: QueryTrait>(query: &Q) -> Result<Self, SeaOrmMySqlError> {
        Self::from_statement(query.build(DbBackend::MySql))
    }
    /// 参数为拥有式 SeaORM Statement；返回无损绑定计划，非 MySQL/未支持 SQL 在执行前拒绝。
    pub fn from_statement(statement: Statement) -> Result<Self, SeaOrmMySqlError> {
        if statement.db_backend != DbBackend::MySql {
            return Err(SeaOrmMySqlError::Input {
                reason: "MySQL statement backend required",
            });
        }
        let recognizer = SqlRecognizerFactory::recognize(&statement.sql)?;
        let parameters = crate::sea_orm_mysql_bindings::SeaOrmMySqlBindings::convert(
            statement.values.map(|values| values.0).unwrap_or_default(),
            recognizer.parameter_count(),
        )?;
        Ok(Self {
            recognizer,
            parameters,
        })
    }
    /// 无参数；返回不可变识别结果；真实表名用于原元数据、锁和 Undo 身份。
    pub fn recognizer(&self) -> &SqlRecognizer {
        &self.recognizer
    }
    /// 无参数；返回原序绑定的不可变精确值，不自动格式化或写入日志。
    pub fn parameters(&self) -> &[FieldValue] {
        &self.parameters
    }
    /// 参数为调用方已拥有的正式本地事务；返回同一业务语句的原始 MySQL 回执，不另开连接或提交。
    pub async fn execute_with_result(
        &self,
        transaction: &mut MySqlLocalTransaction,
    ) -> Result<seata_db_mysql::MySqlStatementResult, SeaOrmMySqlError> {
        Ok(transaction
            .execute_with_result(&self.recognizer, &self.parameters)
            .await?)
    }
    /// 参数为调用方已拥有的正式本地事务；返回影响行数，沿用原毒化、取消、Undo 与提交门槛。
    pub async fn execute(
        &self,
        transaction: &mut MySqlLocalTransaction,
    ) -> Result<u64, SeaOrmMySqlError> {
        Ok(transaction
            .execute(&self.recognizer, &self.parameters)
            .await?)
    }
}
impl fmt::Debug for SeaOrmMySqlStatement {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("SeaOrmMySqlStatement")
            .field("recognizer", &self.recognizer)
            .field("parameter_count", &self.parameters.len())
            .finish_non_exhaustive()
    }
}
