use crate::{ConnectionConfig, ReconnectPolicy, TransportError};
use std::net::SocketAddr;
use tokio::sync::Semaphore;
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
    /// 参数为数值端点、单连接配置、物理槽位数(1..1024)、总入站队列和退避策略；返回已验证配置。
    pub fn new(
        endpoint: SocketAddr,
        connection: ConnectionConfig,
        maximum_connections: usize,
        maximum_inbound: usize,
        reconnect: ReconnectPolicy,
    ) -> Result<Self, TransportError> {
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
}
