use crate::TransportError;
use std::time::{Duration, Instant};

/// 显式开启的写空闲心跳和完整帧读空闲期限，不代表单个PING的确认或认证状态。
/// 对应 Java: org.apache.seata.core.rpc.netty.NettyBaseConfig 的read/write idle参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionIdleConfig {
    writer_idle: Duration,
    reader_idle: Duration,
}
impl ConnectionIdleConfig {
    /// 参数是写空闲间隔、读空闲上限；返回正数、读大于写且单调时钟可表示的配置。
    pub fn new(writer_idle: Duration, reader_idle: Duration) -> Result<Self, TransportError> {
        let now = Instant::now();
        if writer_idle.is_zero()
            || reader_idle <= writer_idle
            || now.checked_add(writer_idle).is_none()
            || now.checked_add(reader_idle).is_none()
        {
            return Err(TransportError::InvalidConfiguration {reason:"idle durations must be positive, finite, and reader idle greater than writer idle"});
        }
        Ok(Self {
            writer_idle,
            reader_idle,
        })
    }
    /// 返回两次完整出站帧之间触发PING的空闲间隔。
    pub const fn writer_idle(self) -> Duration {
        self.writer_idle
    }
    /// 返回两次完整有效入站帧之间允许的最大空闲时间。
    pub const fn reader_idle(self) -> Duration {
        self.reader_idle
    }
}
