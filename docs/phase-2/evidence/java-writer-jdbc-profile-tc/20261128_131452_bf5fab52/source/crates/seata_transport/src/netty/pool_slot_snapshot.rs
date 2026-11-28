use crate::{ConnectionGeneration, ConnectionStats, PoolSlotState, TransportError};
/// 单槽位有界观测记录，不保留历史通道/业务载荷。
/// 对应 Java: NettyClientChannelManager通道元数据职责。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolSlotSnapshot {
    /// 当前槽位阶段。
    pub state: PoolSlotState,
    /// 最后创建的代际，尚未TCP成功时None。
    pub generation: Option<ConnectionGeneration>,
    /// 实际发起连接的次数。
    pub attempts: u64,
    /// 最后一次基础设施失败原因，不含请求/凭据。
    pub last_failure: Option<TransportError>,
    /// Backoff阶段已安排的等待；其他阶段为None。
    pub reconnect_delay: Option<std::time::Duration>,
    /// 最后一次完整初始化报告，包含所有资源的结果；必须与当前修订/状态共同判断。
    pub registration: Option<std::sync::Arc<crate::SessionRegistrationReport>>,
    /// 当前已发布代际的连接计数快照；撤回或重连间隙为None，不包含历史代际。
    /// 该值与池状态跨两把锁采样，可能观察到关闭竞态，须以generation归属。
    pub connection: Option<ConnectionStats>,
}
