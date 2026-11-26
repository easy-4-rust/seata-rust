use crate::MetricsError;
use std::net::SocketAddr;

/// 本机指标端点配置；令牌仅在进程内保存，不参与 Debug 输出。
/// 对应 Java: 监控端点的监听地址和访问控制配置。
pub struct MetricsServerConfig {
    pub(crate) address: SocketAddr,
    pub(crate) token: String,
}

impl MetricsServerConfig {
    /// 参数为回环监听地址和 32–128 字节可见 ASCII Bearer 令牌；返回安全配置。
    pub fn new(address: SocketAddr, token: String) -> Result<Self, MetricsError> {
        if !address.ip().is_loopback() {
            return Err(MetricsError::InvalidConfiguration {
                reason: "metrics listener must bind loopback",
            });
        }
        if !(32..=128).contains(&token.len()) || !token.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(MetricsError::InvalidConfiguration {
                reason: "metrics token must be 32-128 visible ASCII bytes",
            });
        }
        Ok(Self { address, token })
    }
}
