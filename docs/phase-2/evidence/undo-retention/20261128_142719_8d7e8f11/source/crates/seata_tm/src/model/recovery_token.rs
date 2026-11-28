use crate::{TmOperation, Xid};
use std::sync::Arc;
/// 原结束调用签发的不可构造恢复凭证，将管理器来源、XID和意图绑定为一个能力。
/// 对应 Java: DefaultGlobalTransaction的Launcher来源约束；Rust对公开错误增加私有证明。
#[derive(Clone)]
pub struct RecoveryToken {
    authority: Arc<()>,
    xid: Xid,
    operation: TmOperation,
}
impl RecoveryToken {
    pub(crate) fn new(authority: Arc<()>, xid: Xid, operation: TmOperation) -> Self {
        Self {
            authority,
            xid,
            operation,
        }
    }
    pub(crate) fn matches(&self, authority: &Arc<()>, xid: &Xid, operation: TmOperation) -> bool {
        Arc::ptr_eq(&self.authority, authority) && self.xid == *xid && self.operation == operation
    }
}
impl PartialEq for RecoveryToken {
    fn eq(&self, other: &Self) -> bool {
        self.matches(&other.authority, &other.xid, other.operation)
    }
}
impl Eq for RecoveryToken {}
impl std::fmt::Debug for RecoveryToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryToken").finish_non_exhaustive()
    }
}
