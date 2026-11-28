//! 动态报告确认后保持真实事务，供父进程强杀和同实例恢复。
use super::{fixture, tcc_crash_action::TccCrashAction};
use seata_tcc::{BusinessActionContext, TccBusinessAction, TccError, TccTransaction};
use std::{future::Future, pin::Pin};
/// 以原登记值执行业务，持久报告新值后在选定事务边界暂停。
/// 对应 Java: BusinessActionContextUtil#reportContext 的进程故障测试扩展。
#[derive(Debug)]
pub struct TccDynamicCrashAction {
    /// 当前进程保持的真实事务阶段。
    pub stage: String,
}
impl TccBusinessAction for TccDynamicCrashAction {
    fn try_action<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            fixture::Action::default()
                .try_action(context, transaction)
                .await
        })
    }
    fn try_action_with_context<'a>(
        &'a self,
        context: &'a mut BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            fixture::Action::default()
                .try_action(context, transaction)
                .await?;
            let value = serde_json::value::RawValue::from_string("9".into())
                .map_err(|_| TccError::Business)?;
            context.put_action_context("amount", &value)?;
            if self.stage == "report_waiting" {
                fixture::record("uncommitted", serde_json::json!({"stage":self.stage}));
            }
            if !transaction.report_context(context).await? {
                return Err(TccError::Business);
            }
            if self.stage == "try_uncommitted" {
                fixture::record("uncommitted", serde_json::json!({"stage":self.stage}));
                std::future::pending::<()>().await;
            }
            Ok(true)
        })
    }
    fn confirm<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            TccCrashAction {
                stage: self.stage.clone(),
            }
            .confirm(context, transaction)
            .await
        })
    }
    fn cancel<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            TccCrashAction {
                stage: self.stage.clone(),
            }
            .cancel(context, transaction)
            .await
        })
    }
}
