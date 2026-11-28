use super::pending_requests::PendingRequests;
use crate::TransportError;
use std::sync::Arc;

/// 后台future在首次poll前就持有的退出清理能力，panic/abort/正常退出都终止所属会话。
/// 对应 Java: io.netty.channel.ChannelInboundHandlerAdapter#channelInactive 的关闭传播职责。
pub(crate) struct WorkerExitGuard(Arc<PendingRequests>);
impl WorkerExitGuard {
    /// 参数为所属等待表；返回会在Drop时执行幂等终止的守卫。
    pub(crate) fn new(core: Arc<PendingRequests>) -> Self {
        Self(core)
    }
}
impl Drop for WorkerExitGuard {
    fn drop(&mut self) {
        self.0.terminate(TransportError::ConnectionClosed);
    }
}
