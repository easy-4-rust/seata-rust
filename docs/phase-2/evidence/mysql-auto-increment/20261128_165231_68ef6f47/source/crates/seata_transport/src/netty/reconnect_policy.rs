use crate::TransportError;
use std::time::{Duration, Instant};
/// 每槽位正指数退避及稳定连接重置窗口；不授权重放业务请求。
/// 对应 Java: NettyClientChannelManager的连接重建策略；Rust显式限定期限。
#[derive(Debug, Clone, Copy)]
pub struct ReconnectPolicy {
    pub(crate) initial: Duration,
    pub(crate) maximum: Duration,
    pub(crate) stable_after: Duration,
}
impl ReconnectPolicy {
    /// 参数为首次退避、最大退避、成功连接稳定窗口；返回正数且单调时钟可表示的策略。
    pub fn new(
        initial: Duration,
        maximum: Duration,
        stable_after: Duration,
    ) -> Result<Self, TransportError> {
        let now = Instant::now();
        if initial.is_zero()
            || maximum < initial
            || stable_after.is_zero()
            || now.checked_add(maximum).is_none()
            || now.checked_add(stable_after).is_none()
        {
            return Err(TransportError::InvalidConfiguration {
                reason: "positive finite backoff/stable durations and maximum >= initial required",
            });
        }
        Ok(Self {
            initial,
            maximum,
            stable_after,
        })
    }
}
