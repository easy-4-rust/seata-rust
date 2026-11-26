//! 原生Seata TM：显式事务身份、任务作用域及ADR-0008结果分类。
mod api;
mod context;
mod exception;
mod model;
pub use api::{
    GlobalTransaction, TransactionManager, TransactionManagerBuilder, TransactionalTemplate,
};
pub use context::{RootContext, TransactionContext};
pub use exception::TmError;
pub use exception::TransactionError;
pub use model::RecoveryToken;
pub use model::{
    GlobalTransactionRole, Propagation, TransactionConfig, TransactionConfigBuilder, Xid,
};
pub use model::{TmOperation, TransactionCompletion, TransactionOutcome, TransactionResult};
