//! 显式配置、角色选择及生命周期统一入口；各事务模式仍使用原引擎。
mod client_build_error;
mod client_shutdown_error;
mod client_transport_policy;
mod seata_client;

pub use client_build_error::ClientBuildError;
pub use client_shutdown_error::ClientShutdownError;
pub use seata_client::{SeataClient, SeataClientBuilder};
