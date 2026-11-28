//! 在原 TC 回调的实际业务 DML 后保持原事务，供联合强杀验证。
use super::{dynamic_action::TccDynamicAction, fixture};
use seata_tcc::{BusinessActionContext, TccBusinessAction, TccError, TccTransaction};
use std::{future::Future, pin::Pin};
/// 动态 Try 沿用正式报告测试动作，二阶段在指定真实事务切点等待。
/// 对应 Java: FenceHandler 业务事务的联合故障测试扩展。
#[derive(Debug)]
pub struct TccJointAction {
    /// 当前动态业务动作，保留原立即/延迟/精度行为。
    pub dynamic: TccDynamicAction,
    /// 本轮实际事务切点。
    pub cut: String,
}
impl TccJointAction {
    async fn terminal(
        &self,
        phase: &str,
        context: &BusinessActionContext,
        tx: &mut dyn TccTransaction,
    ) -> Result<bool, TccError> {
        let action = fixture::Action::default();
        let value = if phase == "confirm" {
            action.confirm(context, tx).await?
        } else {
            action.cancel(context, tx).await?
        };
        if self.cut == "terminal_uncommitted" {
            fixture::record(
                "joint-terminal-cut",
                serde_json::json!({"cut":self.cut,"phase":phase,"xid":context.xid(),"branch_id":context.branch_id().to_string()}),
            );
            std::future::pending::<()>().await;
        }
        Ok(value)
    }
}
impl TccBusinessAction for TccJointAction {
    fn try_action<'a>(
        &'a self,
        c: &'a BusinessActionContext,
        t: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        self.dynamic.try_action(c, t)
    }
    fn try_action_with_context<'a>(
        &'a self,
        c: &'a mut BusinessActionContext,
        t: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        self.dynamic.try_action_with_context(c, t)
    }
    fn confirm<'a>(
        &'a self,
        c: &'a BusinessActionContext,
        t: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(self.terminal("confirm", c, t))
    }
    fn cancel<'a>(
        &'a self,
        c: &'a BusinessActionContext,
        t: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(self.terminal("cancel", c, t))
    }
}
