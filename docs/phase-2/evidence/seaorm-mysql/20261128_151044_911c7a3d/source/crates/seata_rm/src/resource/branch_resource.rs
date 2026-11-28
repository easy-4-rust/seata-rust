use crate::{BranchIdentity, BranchPhase, BranchResolution, RmError};
use seata_transport::ResourceKey;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

/// 精确资源模式和ID对应的二阶段适配器，必须提供执行和权威恢复两条路径。
/// 对应 Java: org.apache.seata.core.model.ResourceManager；Rust显式异步恢复契约。
///
/// apply可能在Future取消后继续生效。recover返回NotApplied必须持久围栏所有旧尝试；
/// 仅查询不到效果行不满足此条件。数据库效果与Undo的原子性由具体适配器负责。
pub trait BranchResource: Debug + Send + Sync {
    /// 参数为原通道单向清理能力；返回已确认提交的删除数。失败可能已有部分批次生效。
    /// 对应 Java: RMHandlerAT#handle(UndoLogDeleteRequest)；默认明确拒绝不支持的资源，不执行删除。
    fn delete_expired_undo<'a>(
        &'a self,
        _receipt: &'a crate::UndoLogDeleteReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<u64, RmError>> + Send + 'a>> {
        Box::pin(async {
            Err(RmError::Unavailable {
                reason: "resource does not support Undo retention cleanup",
            })
        })
    }
    /// 参数为原连接回调能力；返回资源是否仍有权复用日志 Completed，失败不得重新执行业务。
    /// 默认保留既有可信日志资源的行为；有独立持久控制状态的资源须核验该权威状态。
    fn verify_callback_completion<'a>(
        &'a self,
        _receipt: &'a crate::BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), RmError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }
    /// 参数为原连接回调能力；只读校验后返回日志使用的稳定身份，默认保留全部原字段。
    /// 不得改变原分支键或资源键；不能据此写入资源状态。该校验先于终态日志复用。
    fn callback_identity<'a>(
        &'a self,
        receipt: &'a crate::BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<BranchIdentity, RmError>> + Send + 'a>> {
        Box::pin(async move { Ok(receipt.identity().clone()) })
    }
    /// 无参数；返回可选的回调登记观察器。None 表示不需要此扩展，仍使用完整的原 apply/recover 流程。
    /// 默认保持既有资源实现的行为和接口兼容，不赋予手工 dispatch 回调来源。
    fn callback_observer(&self) -> Option<&dyn crate::BranchCallbackObserver> {
        None
    }
    /// 返回稳定的精确资源键；注册后不可通过内部可变性更换。
    fn key(&self) -> &ResourceKey;
    /// 参数是已写Pending的完整身份和原始方向；返回资源事实，错误一律保留Pending。
    fn apply<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>>;
    /// 参数为原始身份和方向；查询/围栏旧尝试后返回权威事实，不得盲目重做资源动作。
    fn recover<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>>;
}
