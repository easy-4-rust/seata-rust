//! 正式RM控制面及分支持久协调；不把日志标记等同于SQL/Undo已原子完成。
mod exception;
mod manager;
mod model;
mod resource;
mod store;
pub use exception::{RmCallError, RmError};
pub use manager::RmMetricsSampler;
pub use manager::{BranchDispatcher, ResourceClient, ResourceManager, ResourceManagerBuilder};
pub use model::BranchCallbackReceipt;
pub use model::RegisteredBranch;
pub use model::RmBranchMetrics;
pub use model::RmOperation;
pub use model::{BranchIdentity, BranchKey, BranchPhase, BranchRecordState, JournalOptions};
pub use resource::BranchCallbackObserver;
pub use resource::{BranchResolution, BranchResource, ResourceCatalog};
pub use store::{BranchLease, FileBranchStore};
