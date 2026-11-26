//! 原生Seata TM：显式事务身份、任务作用域及ADR-0008结果分类。
mod api;
mod context;
mod exception;
mod model;
#[cfg(all(feature = "application_signals", any(windows, unix)))]
pub use api::ApplicationSignals;
pub use api::{
    DurableGlobalTransaction, DurableTmError, GlobalTransaction, LauncherOrigin,
    LauncherProcessMonitor, LauncherStopChallenge, LauncherStopExchange, LauncherStopKeyRotation,
    LauncherStopSigner, LauncherStopTlsClient, LauncherStopTlsServer, LauncherStopVerifier,
    LauncherStore, RecoveredEndAuthority, RecoveredEndGate, StoppedLauncherOwner,
    TransactionManager, TransactionManagerBuilder, TransactionalTemplate,
};
pub use api::{TransactionAdmission, TransactionPermit};
pub use context::{RootContext, TransactionContext};
pub use exception::AdmissionError;
pub use exception::TmError;
pub use exception::TransactionError;
pub use model::RecoveryToken;
#[cfg(all(feature = "application_signals", any(windows, unix)))]
pub use model::ShutdownSignalKind;
pub use model::{
    GlobalTransactionRole, Propagation, TransactionConfig, TransactionConfigBuilder, Xid,
};
pub use model::{TmOperation, TransactionCompletion, TransactionOutcome, TransactionResult};
