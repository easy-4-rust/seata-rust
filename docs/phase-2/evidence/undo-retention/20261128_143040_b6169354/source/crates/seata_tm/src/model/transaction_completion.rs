use crate::{TransactionOutcome, Xid};
/// 作用域完成情况；参与/无事务路径不伪造TC成功响应。
/// 对应 Java: DefaultGlobalTransaction角色分流及TransactionalTemplate执行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionCompletion {
    /// 本作用域按传播策略没有全局事务。
    Unmanaged,
    /// 仅参与既有事务，不发送全局二阶段。
    Participant(Xid),
    /// 收到原XID的TC响应；状态是否终结仍由outcome决定。
    Observed {
        /// 被操作的原始XID。
        xid: Xid,
        /// 原始TC响应前缀及状态。
        outcome: TransactionOutcome,
    },
}
