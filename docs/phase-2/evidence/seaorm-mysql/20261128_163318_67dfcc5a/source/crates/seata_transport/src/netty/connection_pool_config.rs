use crate::{ConnectionConfig, ReconnectPolicy, TransportError};
use std::net::SocketAddr;
use tokio::sync::Semaphore;

const MAX_QUEUED_FRAME_BYTES: usize = 64 * 1024 * 1024;
/// 单端点raw TCP池的显式物理槽位、入站预算及重连参数。
/// 对应 Java: NettyClientConfig的连接池预算和NettyClientChannelManager端点配置。
#[derive(Debug, Clone, Copy)]
pub struct ConnectionPoolConfig {
    pub(crate) endpoint: SocketAddr,
    pub(crate) connection: ConnectionConfig,
    pub(crate) slots: usize,
    pub(crate) inbound: usize,
    pub(crate) reconnect: ReconnectPolicy,
}
impl ConnectionPoolConfig {
    /// 参数为可连接的数值端点、单连接配置、物理槽位数(1..1024)、总入站队列和退避策略；返回已验证配置。
    pub fn new(
        endpoint: SocketAddr,
        connection: ConnectionConfig,
        maximum_connections: usize,
        maximum_inbound: usize,
        reconnect: ReconnectPolicy,
    ) -> Result<Self, TransportError> {
        if endpoint.port() == 0 || endpoint.ip().is_unspecified() {
            return Err(TransportError::InvalidConfiguration {
                reason: "pool endpoint requires a nonzero port and a specified IP address",
            });
        }
        if maximum_connections == 0
            || maximum_connections > 1024
            || maximum_inbound == 0
            || maximum_inbound > Semaphore::MAX_PERMITS
        {
            return Err(TransportError::InvalidConfiguration {
                reason:
                    "pool requires 1..1024 physical slots and positive bounded inbound capacity",
            });
        }
        if maximum_inbound
            .checked_mul(connection.frame_limits().maximum_frame_length())
            .is_none_or(|bytes| bytes > MAX_QUEUED_FRAME_BYTES)
        {
            return Err(TransportError::InvalidConfiguration {
                reason: "pool inbound worst-case frame bytes exceed 64 MiB",
            });
        }
        Ok(Self {
            endpoint,
            connection,
            slots: maximum_connections,
            inbound: maximum_inbound,
            reconnect,
        })
    }
    /// 返回单端点物理连接槽位上限，不是并发事务数。
    pub const fn maximum_connections(self) -> usize {
        self.slots
    }
    /// 返回该连接池登记并发送请求的数值 TC 端点。
    pub const fn endpoint(self) -> SocketAddr {
        self.endpoint
    }
}
