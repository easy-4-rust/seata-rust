use crate::{ConnectionIdleConfig, RequestIdAllocator, TransportError};
use seata_protocol::v1::FrameLimits;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// 单个原生TCP通道的显式预算和超时；不是TC业务事务期限或认证ready状态。
/// 对应 Java: org.apache.seata.core.rpc.netty.NettyClientConfig 的通道参数职责。
#[derive(Debug, Clone, Copy)]
pub struct ConnectionConfig {
    pub(crate) frames: FrameLimits,
    pub(crate) maximum_pending: usize,
    pub(crate) connect_timeout: Duration,
    pub(crate) write_timeout: Duration,
    pub(crate) first_id: i32,
    pub(crate) id_quota: u64,
    pub(crate) idle: Option<ConnectionIdleConfig>,
}
impl ConnectionConfig {
    /// 参数为帧预算、最大等待调用数、建连期限和单帧写入期限；返回已验证配置。
    /// 队列有界，最大等待数须在1..Semaphore::MAX_PERMITS内并留一个控制帧槽。
    pub fn new(
        frames: FrameLimits,
        maximum_pending: usize,
        connect_timeout: Duration,
        write_timeout: Duration,
    ) -> Result<Self, TransportError> {
        if maximum_pending == 0
            || maximum_pending >= Semaphore::MAX_PERMITS
            || connect_timeout.is_zero()
            || write_timeout.is_zero()
            || Instant::now().checked_add(connect_timeout).is_none()
            || Instant::now().checked_add(write_timeout).is_none()
        {
            return Err(TransportError::InvalidConfiguration {
                reason: "positive bounded capacity and finite positive timeouts required",
            });
        }
        Ok(Self {
            frames,
            maximum_pending,
            connect_timeout,
            write_timeout,
            first_id: 1,
            id_quota: 1u64 << 32,
            idle: None,
        })
    }
    /// 参数为首个signed-int编号和每连接发号额度；返回新配置，额度用尽不自动重放调用。
    pub fn with_id_limits(mut self, first_id: i32, maximum: u64) -> Result<Self, TransportError> {
        RequestIdAllocator::new(first_id, maximum)?;
        self.first_id = first_id;
        self.id_quota = maximum;
        Ok(self)
    }
    /// 参数为已验证的心跳/读空闲配置，返回启用该策略的新配置；原构造器默认关闭。
    pub fn with_idle(mut self, idle: ConnectionIdleConfig) -> Self {
        self.idle = Some(idle);
        self
    }
    /// 返回可选的心跳/读空闲策略；None表示两个计时器均关闭。
    pub const fn idle_config(&self) -> Option<ConnectionIdleConfig> {
        self.idle
    }
    /// 返回帧/头/载荷预算。
    pub const fn frame_limits(&self) -> FrameLimits {
        self.frames
    }
    /// 返回此通道最大等待请求数。
    pub const fn maximum_pending(&self) -> usize {
        self.maximum_pending
    }
    /// 返回建连期限，不包含TC业务期限。
    pub const fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }
    /// 返回单帧写入期限；超时会终止该通道以丢弃可能的半帧。
    pub const fn write_timeout(&self) -> Duration {
        self.write_timeout
    }
}
