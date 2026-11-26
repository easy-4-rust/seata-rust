use crate::mysql_image_reader::MySqlImageReader;
use crate::{cache::MySqlTableMetaReader, DbError};
use crate::{
    exec::{DeleteExecutor, UpdateExecutor},
    mysql::MySqlInsertExecutor,
};
use seata_at::{SqlRecognizer, SqlType, SqlUndoLog};
use seata_datasource::{FieldValue, ImageError, TableMeta, TableRecords};
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
    undo_logs: Vec<SqlUndoLog>,
    undo_bytes: usize,
}
impl MySqlLocalTransaction {
    /// 参数为正式SQL及全部原绑定；只执行一次并准备内存Undo，返回原更新计数；当前没有提交入口。
    /// 对应Java: org.apache.seata.rm.datasource.ConnectionProxy的语句/Undo准备事务边界（部分实现）。
    pub async fn execute(
        &mut self,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<u64, DbError> {
        if self.poisoned {
            return Err(DbError::Poisoned);
        }
        MySqlImageReader::validate_parameters(sql, parameters)?;
        if self.undo_logs.len() >= 128 {
            return Err(ImageError::Budget {
                limit: "128 pending Undo records",
            }
            .into());
        }
        self.poisoned = true;
        // 首次await前把实际事务移入Future，取消也不会留下可复用的半完成业务作用域。
        let mut transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(deadline, async {
            match sql.sql_type() {
                SqlType::Insert => {
                    MySqlInsertExecutor::execute(&mut transaction, &self.database, sql, parameters)
                        .await
                }
                SqlType::Update => {
                    UpdateExecutor::execute(&mut transaction, &self.database, sql, parameters).await
                }
                SqlType::Delete => {
                    DeleteExecutor::execute(&mut transaction, &self.database, sql, parameters).await
                }
            }
        })
        .await;
        match result {
            Ok(result) => {
                self.transaction = Some(transaction);
                let (count, undo) = result?;
                // 本次DML可能已生效但未提交；准备日志预算失败必须保留poison并整体回滚。
                if !(undo.before_image().rows().is_empty() && undo.after_image().rows().is_empty())
                {
                    let bytes = self.undo_bytes.checked_add(undo.value_bytes()).ok_or(
                        ImageError::Budget {
                            limit: "pending Undo bytes overflow",
                        },
                    )?;
                    if bytes > 16_777_216 {
                        return Err(ImageError::Budget {
                            limit: "16MiB pending Undo payload",
                        }
                        .into());
                    }
                    self.undo_logs.push(undo);
                    self.undo_bytes = bytes;
                }
                self.poisoned = false;
                Ok(count)
            }
            Err(_) => {
                drop(transaction);
                Err(DbError::Deadline {
                    operation: "business and images",
                })
            }
        }
    }
    /// 无参数；只读查看事务内准备数据，不表示Undo已落库或业务已提交。
    pub fn undo_logs(&self) -> &[SqlUndoLog] {
        &self.undo_logs
    }
    /// 参数为正式UPDATE/DELETE识别结果与完整原绑定；返回同事务持锁完整前镜像，不执行业务SQL。
    /// 对应Java: org.apache.seata.rm.datasource.exec.UpdateExecutor#beforeImage及DeleteExecutor；部分实现。
    pub async fn before_image(
        &mut self,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<TableRecords, DbError> {
        if self.poisoned {
            return Err(DbError::Poisoned);
        }
        MySqlImageReader::validate_request(sql, parameters)?;
        self.poisoned = true;
        let mut transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(
            deadline,
            MySqlImageReader::read(&mut transaction, &self.database, sql, parameters),
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
                    operation: "before image",
                })
            }
        }
    }
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
            undo_logs: Vec::new(),
            undo_bytes: 0,
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
