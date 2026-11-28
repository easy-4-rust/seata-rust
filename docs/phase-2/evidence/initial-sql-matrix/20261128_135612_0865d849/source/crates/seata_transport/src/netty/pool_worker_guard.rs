use super::pool_core::PoolCore;
use crate::TransportError;
use std::sync::Arc;
/// pool监督任务的预构造退出守卫；即使future尚未poll或panic，也不留孤立开放池。
/// 对应 Java: NettyClientChannelManager关闭监督职责，Rust以RAII保证。
pub(crate) struct PoolWorkerGuard {
    core: Arc<PoolCore>,
}
impl PoolWorkerGuard {
    /// 参数是共享池状态，返回独占于一个监督future的退出守卫。
    pub(crate) fn new(core: Arc<PoolCore>) -> Self {
        Self { core }
    }
}
impl Drop for PoolWorkerGuard {
    fn drop(&mut self) {
        self.core.stop(TransportError::InternalState {
            reason: "pool supervisor exited",
        });
    }
}
