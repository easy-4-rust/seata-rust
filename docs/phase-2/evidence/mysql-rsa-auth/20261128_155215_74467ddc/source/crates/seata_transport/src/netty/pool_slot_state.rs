/// raw池槽位状态；Connected不代表身份认证或RM资源ready。
/// 对应 Java: NettyClientChannelManager的通道连接/回收状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSlotState {
    /// 尚在发起或等待TCP建立。
    Connecting,
    /// raw通道开放；业务初始化必须另行完成。
    Connected,
    /// 正在登记身份/当前缓存资源修订，不对业务获取开放。
    Registering,
    /// 当前代际及缓存修订的全部所需登记已接受。
    Ready,
    /// 身份或资源登记失败，保存逐步骤结果并等待新尝试。
    Degraded,
    /// 旧通道已撤回选择，正在清理/等待下次连接。
    Backoff,
    /// 池永久关闭，该槽位不再连接。
    Stopped,
}
