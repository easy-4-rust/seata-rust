//! TCC 原上下文与事务内 Fence 决策；不从值对象或缺行推导已提交、TC 授权。
mod api;
mod exception;
mod fence;
pub use api::BusinessActionContext;
pub use exception::TccError;
pub use fence::{CommonFenceStatus, FenceDecision, FenceHandler, FenceOperation};
