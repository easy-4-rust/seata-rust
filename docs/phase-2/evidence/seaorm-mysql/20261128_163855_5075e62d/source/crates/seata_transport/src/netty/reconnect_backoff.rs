use crate::ReconnectPolicy;
use std::time::Duration;
/// 单槽位独占的饱和指数退避；短暂连接成功不能使抖动重连退化为密集循环。
/// 对应 Java: NettyClientChannelManager连接重试调度职责。
pub(crate) struct ReconnectBackoff {
    policy: ReconnectPolicy,
    next: Duration,
}
impl ReconnectBackoff {
    /// 参数是已验证策略；返回首次等待为initial的独占状态。
    pub(crate) fn new(policy: ReconnectPolicy) -> Self {
        Self {
            policy,
            next: policy.initial,
        }
    }
    /// 返回下一次正退避并饱和推进到最大值，不做整数环绕或零间隔重试。
    pub(crate) fn next_delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = self.next.saturating_mul(2).min(self.policy.maximum);
        delay
    }
    /// 参数是刚结束连接的持续时间；只有达到稳定窗口才重置退避。
    pub(crate) fn connection_ended(&mut self, lifetime: Duration) {
        if lifetime >= self.policy.stable_after {
            self.next = self.policy.initial;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::ReconnectBackoff;
    use crate::ReconnectPolicy;
    use std::time::Duration;
    #[test]
    fn backoff_caps_without_wrap_and_short_lived_connections_do_not_reset() {
        let mut state = ReconnectBackoff::new(
            ReconnectPolicy::new(
                Duration::from_millis(10),
                Duration::from_millis(35),
                Duration::from_secs(1),
            )
            .unwrap_or_else(|error| panic!("{error}")),
        );
        assert_eq!(state.next_delay(), Duration::from_millis(10));
        state.connection_ended(Duration::from_millis(999));
        assert_eq!(state.next_delay(), Duration::from_millis(20));
        assert_eq!(state.next_delay(), Duration::from_millis(35));
        for _ in 0..100 {
            assert_eq!(state.next_delay(), Duration::from_millis(35));
        }
        state.connection_ended(Duration::from_secs(1));
        assert_eq!(state.next_delay(), Duration::from_millis(10));
    }
}
