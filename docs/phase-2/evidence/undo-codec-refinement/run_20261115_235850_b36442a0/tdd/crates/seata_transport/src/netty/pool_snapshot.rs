use crate::{PoolSlotSnapshot, PoolSlotState, TransportError};
/// 固定槽位池的一致元数据快照，内存不随重连次数增长。
/// 对应 Java: NettyClientChannelManager池监控职责。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolSnapshot {
    /// 池永久终止标志。
    pub closed: bool,
    /// 第一个池级终止原因。
    pub last_failure: Option<TransportError>,
    /// 配置的固定槽位记录。
    pub slots: Vec<PoolSlotSnapshot>,
}
impl PoolSnapshot {
    /// 返回标记为raw已连接的槽位数；不推断资源注册ready。
    pub fn connected(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| {
                matches!(
                    slot.state,
                    PoolSlotState::Connected | PoolSlotState::Registering | PoolSlotState::Ready
                )
            })
            .count()
    }
    /// 返回由池核对当前代际/资源修订后仍ready的槽位数；raw通道不计入。
    pub fn ready(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.state == PoolSlotState::Ready)
            .count()
    }
}
