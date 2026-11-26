use crate::TransactionContext;
use std::future::Future;
tokio::task_local! {static CURRENT:Option<TransactionContext>;}
/// 按future每次poll绑定的事务上下文，替代跨await不安全的线程局部变量。
/// 对应 Java: org.apache.seata.core.context.RootContext；Rust使用词法作用域自动恢复。
#[derive(Debug)]
pub struct RootContext;
impl RootContext {
    pub(crate) async fn scope_owned<F: Future>(
        context: Option<TransactionContext>,
        future: F,
    ) -> F::Output {
        CURRENT.scope(context, future).await
    }
    /// 返回当前任务作用域的不可变快照，无作用域时为None，不读取其他任务。
    pub fn current() -> Option<TransactionContext> {
        CURRENT.try_with(Clone::clone).ok().flatten()
    }
    /// 返回用于显式跨任务传播的快照；即便来源为Launcher也降为Participant。
    pub fn capture() -> Option<TransactionContext> {
        Self::current().map(TransactionContext::into_participant)
    }
    /// 参数为显式传播快照和future；返回future结果，所有传入身份均以Participant运行。
    /// future取消/Drop/panic均同步恢复外层，不启动网络清理或继承到spawn任务。
    pub async fn scope<F: Future>(context: Option<TransactionContext>, future: F) -> F::Output {
        CURRENT
            .scope(context.map(TransactionContext::into_participant), future)
            .await
    }
}
