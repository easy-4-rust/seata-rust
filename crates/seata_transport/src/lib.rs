//! 原生Seata V1连接层；流解码和请求编号由连接代际独占，协议字段由seata-protocol解释。
mod exception;
mod netty;
pub use netty::{
    RegistrationAttempt, RegistrationIdentity, RegistrationRole, RegistrationStatus, ResourceKey,
    ResourceRegistrationResult, ResourceRegistry, ResourceRegistrySnapshot, SessionRegistration,
    SessionRegistrationReport,
};

pub use exception::{CallError, TransportError};
pub use netty::{
    ConnectionConfig, ConnectionGeneration, ConnectionHandle, ConnectionIdleConfig,
    ConnectionStats, FrameDecoder, RequestIdAllocator, TcpConnection,
};
pub use netty::{
    ConnectionPool, ConnectionPoolConfig, PoolCounters, PoolHandle, PoolInbound, PoolSampler,
    PoolSlotSnapshot, PoolSlotState, PoolSnapshot, ReconnectPolicy,
};

#[cfg(feature = "tls")]
pub use netty::TlsClientConfig;
