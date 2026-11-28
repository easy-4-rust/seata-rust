use crate::{BranchCallbackReceipt, RmError};
use std::{fmt::Debug, future::Future, pin::Pin};

/// 在文件 Pending 之后接受原连接回调能力的可选资源登记扩展。
/// 对应 Java: 无直接对象；对 org.apache.seata.rm.AbstractRMHandler 回调来源的 Rust 持久交接扩展。
pub trait BranchCallbackObserver: Debug + Send + Sync {
    /// 参数为正式 RM 入口产生的回调能力；确认登记后返回成功，未知或失败必须保留 Pending。
    /// 不得把该能力合成为 BranchRegister 成功回执，不得重发原注册或业务 SQL。
    fn observe<'a>(
        &'a self,
        receipt: &'a BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), RmError>> + Send + 'a>>;
}
