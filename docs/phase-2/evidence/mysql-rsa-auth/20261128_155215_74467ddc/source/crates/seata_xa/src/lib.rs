//! 独立 XA 引擎的原身份模型，不从裸 XID 推导 TC 权限或完成状态。
mod xa_branch_xid;
mod xa_error;
mod xa_operation;
mod xa_xid;
pub use xa_branch_xid::XaBranchXid;
pub use xa_error::XaError;
pub use xa_operation::XaOperation;
pub use xa_xid::XaXid;
