use crate::{rbatis_mysql_bindings::RbatisMySqlBindings, RbatisMySqlError};
use rbs::Value;
use seata_at::{SqlRecognizer, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use seata_db_mysql::{MySqlLocalTransaction, MySqlStatementResult};
use std::fmt;
/// 已完整认证的原 SQL 与原序参数；不拥有连接或发布提交能力。
/// 对应 Java: 无直接对象；PreparedStatementProxy 的 Rust rbatis 输入适配。
pub struct RbatisMySqlStatement {
    recognizer: SqlRecognizer,
    parameters: Vec<FieldValue>,
}
impl RbatisMySqlStatement {
    /// 参数为宏生成的完整 SQL 与拥有式绑定；返回原事务可执行的密封 DML 计划。
    pub fn from_sql(sql: &str, values: Vec<Value>) -> Result<Self, RbatisMySqlError> {
        let recognizer = match SqlRecognizerFactory::recognize(sql) {
            Ok(plan) => plan,
            Err(error) => {
                RbatisMySqlBindings::discard(values);
                return Err(error.into());
            }
        };
        let parameters = RbatisMySqlBindings::convert(values, recognizer.parameter_count())?;
        Ok(Self {
            recognizer,
            parameters,
        })
    }
    /// 无参数；返回原正式识别器，不内插绑定或生成另一条业务 SQL。
    pub fn recognizer(&self) -> &SqlRecognizer {
        &self.recognizer
    }
    /// 无参数；返回不可变的精确绑定序列。
    pub fn parameters(&self) -> &[FieldValue] {
        &self.parameters
    }
    /// 参数为原拥有式本地事务；返回原业务语句回执，沿用 Undo/预算/取消/毒化边界。
    pub async fn execute_with_result(
        &self,
        transaction: &mut MySqlLocalTransaction,
    ) -> Result<MySqlStatementResult, RbatisMySqlError> {
        Ok(transaction
            .execute_with_result(&self.recognizer, &self.parameters)
            .await?)
    }
}
impl fmt::Debug for RbatisMySqlStatement {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("RbatisMySqlStatement")
            .field("parameter_count", &self.parameters.len())
            .finish_non_exhaustive()
    }
}
