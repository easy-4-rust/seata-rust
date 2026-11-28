use super::pending_requests::PendingRequests;
use std::sync::Arc;

/// 调用future的取消清理能力；独占ID，Drop不需要异步运行或额外任务。
/// 对应 Java: MessageFuture超时/取消清理职责；Rust补充future取消安全。
pub(crate) struct PendingRequestGuard {
    core: Arc<PendingRequests>,
    id: i32,
}
impl PendingRequestGuard {
    /// 参数为该会话的等待表及已注册ID；返回非Clone的取消所有者。
    pub(crate) fn new(core: Arc<PendingRequests>, id: i32) -> Self {
        Self { core, id }
    }
}
impl Drop for PendingRequestGuard {
    fn drop(&mut self) {
        self.core.cancel(self.id);
    }
}
