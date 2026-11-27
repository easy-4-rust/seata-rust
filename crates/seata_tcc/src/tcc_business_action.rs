use crate::{BusinessActionContext, TccError, TccTransaction};
use std::{fmt::Debug, future::Future, pin::Pin};
/// 显式三阶段业务对象；同库效果必须通过提供的原事务能力执行。
/// 对应 Java: org.apache.seata.rm.tcc.TCCResource 的 prepare/commit/rollback 目标方法。
/// 返回 false/错误时适配器回滚 Fence 和业务；已确认的独立上下文报告不随本地事务回滚。
/// 外部服务等效果不属于此本地事务。
pub trait TccBusinessAction: Debug + Send + Sync {
    /// 参数为可变业务上下文和原 Try 事务；默认委托既有只读 Try，实现可按需覆盖动态参数交接。
    fn try_action_with_context<'a>(
        &'a self,
        context: &'a mut BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        self.try_action(context, transaction)
    }
    /// 参数为原上下文和 Try 事务能力；返回业务是否接受本次 Try，不自行提交或派生分支。
    fn try_action<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>>;
    /// 参数为原上下文和 Confirm 事务能力；返回业务是否成功，与 Fence 终态同事务提交。
    fn confirm<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>>;
    /// 参数为原上下文和 Cancel 事务能力；返回业务是否成功，空 Cancel 由存储层跳过业务。
    fn cancel<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>>;
}
