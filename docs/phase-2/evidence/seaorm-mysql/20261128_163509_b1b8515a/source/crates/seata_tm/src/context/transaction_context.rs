use crate::{GlobalTransactionRole, Xid};
use tokio::time::Instant;
/// 不可变的任务事务快照，嵌套或跨任务传播只能取得参与方角色。
/// 对应 Java: org.apache.seata.core.context.RootContext的事务上下文职责。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionContext {
    pub(crate) xid: Xid,
    pub(crate) role: GlobalTransactionRole,
    pub(crate) deadline: Option<Instant>,
}
impl TransactionContext {
    /// 参数是已验证的外部XID；返回不拥有全局二阶段决策权、期限未知的参与方快照。
    pub const fn participant(xid: Xid) -> Self {
        Self {
            xid,
            role: GlobalTransactionRole::Participant,
            deadline: None,
        }
    }
    /// 返回原XID借用。
    pub const fn xid(&self) -> &Xid {
        &self.xid
    }
    /// 返回此作用域角色，不授权调用方生成新的Launcher能力。
    pub const fn role(&self) -> GlobalTransactionRole {
        self.role
    }
    /// 返回已知的业务绝对期限，外部传播不能伪造为新的完整期限。
    pub const fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
    pub(crate) fn into_participant(mut self) -> Self {
        self.role = GlobalTransactionRole::Participant;
        self
    }
}
