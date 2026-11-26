use crate::RmError;

/// 本地持久协调状态；状态本身不能证明数据库资源事实。
/// 对应 Java: RM二阶段处理/重试语义；Rust日志扩展，不复用TC全局状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BranchRecordState {
    /// 已记录可能执行；必须恢复确认，不能盲目重放。
    Pending = 1,
    /// 资源权威确认本次无效果，允许同方向新尝试。
    RetrySafe = 2,
    /// 资源确认已执行，且本地完成记录已同步。
    Completed = 3,
    /// 资源权威拒绝，持久保存不可重试结果。
    Rejected = 4,
}
impl BranchRecordState {
    /// 将参数日志码解为状态；未知码返回损坏错误，不猜测兼容值。
    pub(crate) fn decode(value: u8) -> Result<Self, RmError> {
        match value {
            1 => Ok(Self::Pending),
            2 => Ok(Self::RetrySafe),
            3 => Ok(Self::Completed),
            4 => Ok(Self::Rejected),
            _ => Err(RmError::Journal {
                reason: "unknown record state",
            }),
        }
    }
    /// 参数为原状态和目标状态；返回是否满足持久协调迁移或幂等重复规则。
    pub(crate) fn permits(previous: Option<Self>, next: Self) -> bool {
        match previous {
            None => next == Self::Pending,
            Some(value) if value == next => true,
            Some(Self::Pending) => {
                matches!(next, Self::RetrySafe | Self::Completed | Self::Rejected)
            }
            Some(Self::RetrySafe) => next == Self::Pending,
            _ => false,
        }
    }
}
