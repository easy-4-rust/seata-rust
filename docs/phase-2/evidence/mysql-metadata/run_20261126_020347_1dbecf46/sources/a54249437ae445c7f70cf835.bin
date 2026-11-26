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
