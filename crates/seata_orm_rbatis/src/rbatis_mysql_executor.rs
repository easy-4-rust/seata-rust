use crate::{
    rbatis_mysql_bindings::RbatisMySqlBindings, rbatis_mysql_read_plan::RbatisMySqlReadPlan,
    rbatis_mysql_rows::RbatisMySqlRows, RbatisMySqlError, RbatisMySqlStatement,
};
use futures_core::future::BoxFuture;
use rbatis::{
    executor::{Executor, RBatisRef},
    rbdc::db::ExecResult,
    Error, RBatis,
};
use rbs::Value;
use seata_db_mysql::MySqlLocalTransaction;
use std::{
    fmt,
    sync::atomic::{AtomicI64, Ordering},
};
use tokio::sync::Mutex;
static NEXT_ID: AtomicI64 = AtomicI64::new(1);
/// 独占拥有原 AT 事务的真实 rbatis 执行器，消费后交还同一事务供原准备/提交入口使用。
/// 对应 Java: 无直接对象；ConnectionProxy 的 Rust ORM 拥有式执行扩展。
pub struct RbatisMySqlExecutor {
    transaction: Mutex<MySqlLocalTransaction>,
    engine: RBatis,
    id: i64,
}
impl RbatisMySqlExecutor {
    /// 参数为已拥有的原 AT 事务；返回不连接另一数据库或启用默认日志的执行器。
    /// ```compile_fail
    /// fn forbidden(executor: &seata_orm_rbatis::RbatisMySqlExecutor) {
    ///     let _ = executor.commit();
    /// }
    /// ```
    pub fn new(transaction: MySqlLocalTransaction) -> Self {
        Self {
            transaction: Mutex::new(transaction),
            engine: RBatis::default(),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
    /// 消费执行器；返回原事务，继续原准备、提交授权或回滚，不克隆连接。
    pub fn into_transaction(self) -> MySqlLocalTransaction {
        self.transaction.into_inner()
    }
    fn admission(&self) -> Result<(), RbatisMySqlError> {
        if self.engine.pool.get().is_some() || !self.engine.intercepts.is_empty() {
            return Err(RbatisMySqlError::Input {
                reason: "configured ORM pool/interceptors are not admitted",
            });
        }
        Ok(())
    }
}
impl RBatisRef for RbatisMySqlExecutor {
    fn rb_ref(&self) -> &RBatis {
        &self.engine
    }
    fn driver_type(&self) -> Result<&str, Error> {
        Ok("mysql")
    }
}
impl Executor for RbatisMySqlExecutor {
    fn id(&self) -> i64 {
        self.id
    }
    fn exec(&self, sql: &str, values: Vec<Value>) -> BoxFuture<'_, Result<ExecResult, Error>> {
        // 同步消耗所有未经认证的 RBS 容器，避免未轮询 future 的递归析构。
        let plan = RbatisMySqlStatement::from_sql(sql, values);
        Box::pin(async move {
            let plan = plan.map_err(|e| Error::from(e.to_string()))?;
            let mut transaction = self.transaction.lock().await;
            self.admission().map_err(|e| Error::from(e.to_string()))?;
            let result = plan
                .execute_with_result(&mut transaction)
                .await
                .map_err(|e| Error::from(e.to_string()))?;
            Ok(ExecResult {
                rows_affected: result.rows_affected(),
                last_insert_id: Value::U64(result.last_insert_id()),
            })
        })
    }
    fn query(&self, sql: &str, values: Vec<Value>) -> BoxFuture<'_, Result<Value, Error>> {
        let plan = (|| -> Result<_, RbatisMySqlError> {
            let plan = match RbatisMySqlReadPlan::recognize(sql) {
                Ok(plan) => plan,
                Err(error) => {
                    RbatisMySqlBindings::discard(values);
                    return Err(error.into());
                }
            };
            let values = RbatisMySqlBindings::convert(values, plan.parameter_count())?;
            plan.validate(&values)?;
            Ok((plan, values))
        })();
        Box::pin(async move {
            let (plan, values) = plan.map_err(|e| Error::from(e.to_string()))?;
            let mut transaction = self.transaction.lock().await;
            self.admission().map_err(|e| Error::from(e.to_string()))?;
            let rows = plan
                .execute(&mut transaction, &values)
                .await
                .map_err(|e| Error::from(e.to_string()))?;
            RbatisMySqlRows::convert(rows).map_err(|e| Error::from(e.to_string()))
        })
    }
}
impl fmt::Debug for RbatisMySqlExecutor {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("RbatisMySqlExecutor")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
