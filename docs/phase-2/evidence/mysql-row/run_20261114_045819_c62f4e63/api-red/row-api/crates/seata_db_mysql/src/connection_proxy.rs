use crate::{cache::MySqlTableMetaReader, DbError};
use seata_datasource::TableMeta;
use sqlx::{MySql, Transaction};
use std::{fmt, time::Duration};
use tokio::time::{timeout_at, Instant};

/// 单一连接的本地事务作用域；失败或取消后只允许回滚/Drop，当前不暴露绕过Undo的提交入口。
/// 对应 Java: org.apache.seata.rm.datasource.ConnectionProxy的本地事务/元数据基础路径（部分实现）。
pub struct MySqlLocalTransaction {
    transaction: Option<Transaction<'static, MySql>>,
    database: String,
    poisoned: bool,
    operation_timeout: Duration,
}
impl MySqlLocalTransaction {
    /// 参数为独占SQLx事务及固定schema；返回可取消但不可共享连接的作用域。
    pub(crate) fn new(
        transaction: Transaction<'static, MySql>,
        database: String,
        operation_timeout: Duration,
    ) -> Self {
        Self {
            transaction: Some(transaction),
            database,
            poisoned: false,
            operation_timeout,
        }
    }
    /// 参数为原单表名；在同事务持有MDL后读取完整元数据，错误或取消保持poison。
    pub async fn table_meta(&mut self, table: &str) -> Result<TableMeta, DbError> {
        if self.poisoned {
            return Err(DbError::Poisoned);
        }
        // 在首次await之前立起失败状态，Future被取消也不能继续复用半完成作用域。
        self.poisoned = true;
        // Future暂时拥有整个事务；调用被取消时事务立即Drop，而不是遗留在外层对象中。
        let mut transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(
            deadline,
            MySqlTableMetaReader::fetch(&mut transaction, &self.database, table),
        )
        .await;
        match result {
            Ok(result) => {
                self.transaction = Some(transaction);
                if result.is_ok() {
                    self.poisoned = false;
                }
                result
            }
            Err(_) => {
                drop(transaction);
                Err(DbError::Deadline {
                    operation: "table metadata",
                })
            }
        }
    }
    /// 无参数；显式回滚并归还连接，返回原驱动分类；Drop仍由SQLx负责安全回滚。
    pub async fn rollback(mut self) -> Result<(), DbError> {
        let transaction = self.transaction.take().ok_or(DbError::Closed)?;
        timeout_at(
            Instant::now() + self.operation_timeout,
            transaction.rollback(),
        )
        .await
        .map_err(|_| DbError::Deadline {
            operation: "rollback",
        })?
        .map_err(|error| DbError::driver("rollback", error))
    }
}
impl fmt::Debug for MySqlLocalTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlLocalTransaction")
            .field("poisoned", &self.poisoned)
            .field("open", &self.transaction.is_some())
            .finish_non_exhaustive()
    }
}
