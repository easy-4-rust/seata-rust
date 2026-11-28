use super::pool_core::PoolCore;
use crate::PoolSnapshot;
use std::sync::Arc;

/// 可克隆的池只读采样能力；不授予请求、重连或关闭权限。
/// 对应 Java: NettyClientChannelManager 的只读监控视图。
#[derive(Clone)]
pub struct PoolSampler {
    pub(crate) core: Arc<PoolCore>,
}

impl PoolSampler {
    /// 返回当前槽位与进程生命周期累计计数；采样与网络事件可能并发。
    pub fn snapshot(&self) -> PoolSnapshot {
        self.core.snapshot()
    }
}
