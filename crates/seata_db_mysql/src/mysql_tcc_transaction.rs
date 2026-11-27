use crate::{
    cache::MySqlTableMetaReader, current_table_constraints::CurrentTableConstraints,
    mysql_image_reader::MySqlImageReader, mysql_tcc_fence_store::MySqlTccFenceStore, DbError,
    MySqlDataSource,
};
use seata_at::SqlRecognizerFactory;
use seata_datasource::{FieldValue, TableMeta};
use seata_tcc::{TccError, TccTransaction};
use sqlx::{MySql, Transaction};
use std::{fmt, future::Future, pin::Pin};
/// Fence 与业务共享的独占连接作用域；取消或任何 DML 错误后原作用域不能继续提交。
/// 对应 Java: SpringFenceHandler 的 TransactionTemplate 作用域；Rust 受限能力与封闭扩展。
pub(crate) struct MySqlTccTransaction {
    source: MySqlDataSource,
    transaction: Option<Transaction<'static, MySql>>,
}
impl fmt::Debug for MySqlTccTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlTccTransaction")
            .field("active", &self.transaction.is_some())
            .finish_non_exhaustive()
    }
}
impl MySqlTccTransaction {
    /// 参数为原数据源；取得关闭即丢弃的控制连接，并验证当前 Fence 结构和实例。
    pub(crate) async fn begin(source: &MySqlDataSource) -> Result<Self, DbError> {
        let mut connection = source
            .control_pool()
            .acquire()
            .await
            .map_err(|e| DbError::acquisition("TCC transaction acquire", e))?;
        connection.close_on_drop();
        let mut transaction = Transaction::begin(connection, None)
            .await
            .map_err(|e| DbError::driver("TCC transaction begin", e))?;
        MySqlTccFenceStore::validate(&mut transaction, source).await?;
        Ok(Self {
            source: source.clone(),
            transaction: Some(transaction),
        })
    }
    /// 无参数；仅适配器内部借用原事务，业务接口不暴露此入口。
    pub(crate) fn control(&mut self) -> Result<&mut Transaction<'static, MySql>, DbError> {
        self.transaction.as_mut().ok_or(DbError::Input {
            reason: "TCC transaction is closed",
        })
    }
    /// 无参数；业务和 Fence 全部成功后消费式提交；回包未知不宣称成功。
    pub(crate) async fn commit(self) -> Result<(), DbError> {
        let mut started = false;
        self.commit_tracked(&mut started).await
    }
    /// 参数为当前 Try 的发送边界；仅在验证成功并即将发送 COMMIT 前置 true，取消不能回退该事实。
    pub(crate) async fn commit_tracked(mut self, started: &mut bool) -> Result<(), DbError> {
        let mut tx = self.transaction.take().ok_or(DbError::Input {
            reason: "TCC transaction is poisoned",
        })?;
        let current:(String,String,i64)=sqlx::query_as("SELECT @@server_uuid,DATABASE(),CAST(@@GLOBAL.innodb_flush_log_at_trx_commit AS SIGNED)").fetch_one(&mut *tx).await.map_err(|e|DbError::driver("TCC commit identity",e))?;
        if current.0 != self.source.server_uuid.as_ref()
            || current.1 != self.source.database()
            || current.2 != 1
        {
            return Err(DbError::RecoveryConflict);
        }
        *started = true;
        tx.commit()
            .await
            .map_err(|e| DbError::driver("TCC business and Fence commit", e))
    }
    async fn execute_inner(
        &mut self,
        sql: &str,
        parameters: &[FieldValue],
    ) -> Result<u64, DbError> {
        // 首次 await 前移走事务；错误、业务捕获错误或 Future 取消都不能重新取回它。
        let mut tx = self.transaction.take().ok_or(DbError::Input {
            reason: "TCC transaction is poisoned",
        })?;
        if sql.len() > 65536 || parameters.len() > 512 {
            return Err(DbError::Input {
                reason: "TCC DML budget",
            });
        }
        let plan = SqlRecognizerFactory::recognize(sql).map_err(|_| DbError::Input {
            reason: "TCC DML is unsupported",
        })?;
        let table = plan.table_name();
        if table.eq_ignore_ascii_case("undo_log")
            || table.eq_ignore_ascii_case("tcc_fence_log")
            || table.to_ascii_lowercase().starts_with("seata_rs_")
        {
            return Err(DbError::Input {
                reason: "TCC internal table is reserved",
            });
        }
        MySqlImageReader::validate_parameters(&plan, parameters)?;
        let arguments = MySqlImageReader::arguments(parameters)?;
        let quoted = TableMeta::quote_identifier(table)?;
        sqlx::query(&format!("SELECT 1 FROM {quoted} LIMIT 0 FOR UPDATE"))
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::driver("TCC business metadata lock", e))?;
        MySqlTableMetaReader::verify_current_engine(&mut tx, table).await?;
        CurrentTableConstraints::verify(
            &mut tx,
            &self.source.control_pool().connect_options(),
            self.source.database(),
            table,
        )
        .await?;
        let result = sqlx::query_with(plan.sql(), arguments)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::driver("TCC business DML", e))?
            .rows_affected();
        self.transaction = Some(tx);
        Ok(result)
    }
}
impl TccTransaction for MySqlTccTransaction {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        parameters: &'a [FieldValue],
    ) -> Pin<Box<dyn Future<Output = Result<u64, TccError>> + Send + 'a>> {
        Box::pin(async move {
            self.execute_inner(sql, parameters)
                .await
                .map_err(|_| TccError::TransactionUnavailable)
        })
    }
}
