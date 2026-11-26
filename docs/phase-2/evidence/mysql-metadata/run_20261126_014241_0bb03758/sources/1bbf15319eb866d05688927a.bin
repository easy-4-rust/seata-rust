use crate::{BranchIdentity, BranchPhase};

/// 正式 RM 连接整批校验后产生的回调能力；不冒充原 BranchRegister 成功响应。
/// 对应 Java: org.apache.seata.rm.AbstractRMHandler#doBranchRollback/doBranchCommit 的原连接入口；Rust 增加受控交接对象。
/// 回调没有锁键字段，因此此类型不提供锁键，也不能转换为 RegisteredBranch。
///
/// ```compile_fail
/// let _forged_constructor = seata_rm::BranchCallbackReceipt::received;
/// ```
#[derive(Debug)]
pub struct BranchCallbackReceipt {
    identity: BranchIdentity,
    phase: BranchPhase,
}
impl BranchCallbackReceipt {
    /// 参数为已校验的原身份和 TC 方向；返回仅供当前连接回调处理器构造的能力。
    pub(crate) fn received(identity: BranchIdentity, phase: BranchPhase) -> Self {
        Self { identity, phase }
    }
    /// 无参数；返回回调携带的完整原身份借用，不从申请或缺失响应猜测编号。
    pub fn identity(&self) -> &BranchIdentity {
        &self.identity
    }
    /// 无参数；返回原 TC 的提交或回滚方向。
    pub const fn phase(&self) -> BranchPhase {
        self.phase
    }
}
