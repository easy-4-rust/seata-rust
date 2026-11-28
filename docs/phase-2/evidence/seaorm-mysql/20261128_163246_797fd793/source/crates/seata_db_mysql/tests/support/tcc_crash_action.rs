//! 在真实业务 DML 之后保持原事务的进程故障动作。
use super::fixture;
use seata_tcc::{BusinessActionContext, TccBusinessAction, TccError, TccTransaction};
use std::{future::Future, pin::Pin};

/// 委托真实业务后保持指定阶段事务，供父进程验证锁并强杀自有写入者。
/// 对应 Java: 无直接对象；TCC 原业务事务故障测试扩展。
#[derive(Debug)]
pub struct TccCrashAction {
    /// 当前自有写入进程要保持的事务阶段。
    pub stage: String,
}
impl TccCrashAction {
    async fn invoke(
        &self,
        phase: &str,
        context: &BusinessActionContext,
        transaction: &mut dyn TccTransaction,
    ) -> Result<bool, TccError> {
        let action = fixture::Action::default();
        let value = match phase {
            "try" => action.try_action(context, transaction).await?,
            "confirm" => action.confirm(context, transaction).await?,
            _ => action.cancel(context, transaction).await?,
        };
        if self.stage == format!("{phase}_uncommitted") {
            fixture::record("uncommitted", serde_json::json!({"stage":self.stage}));
            std::future::pending::<()>().await;
        }
        Ok(value)
    }
}
impl TccBusinessAction for TccCrashAction {
    fn try_action<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(self.invoke("try", context, transaction))
    }
    fn confirm<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(self.invoke("confirm", context, transaction))
    }
    fn cancel<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(self.invoke("cancel", context, transaction))
    }
}
