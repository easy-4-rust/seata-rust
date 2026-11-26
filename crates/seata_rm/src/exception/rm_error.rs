use std::io::ErrorKind;

/// RM本地校验与持久协调错误；诊断不包含XID、资源地址、应用数据或文件路径。
/// 对应 Java: TransactionException；增加Rust日志容量及恢复失败分类。
#[derive(Debug, thiserror::Error)]
pub enum RmError {
    /// 配置或分支身份不满足高层契约。
    #[error("invalid RM input: {reason}")]
    Invalid {
        /// 固定说明，不拼接业务输入。
        reason: &'static str,
    },
    /// 同一XID/BranchId不能更换资源、应用数据或二阶段方向。
    #[error("branch identity or phase conflicts with its durable record")]
    IdentityConflict,
    /// 本地文件操作失败；不把未经确认的写入当作完成。
    #[error("branch journal I/O failed: {kind:?}")]
    Storage {
        /// 操作系统错误类别。
        kind: ErrorKind,
    },
    /// 已有日志损坏、版本不支持或状态迁移非法。
    #[error("branch journal rejected: {reason}")]
    Journal {
        /// 固定说明。
        reason: &'static str,
    },
    /// 容量耗尽，不驱逐已完成分支的去重记录。
    #[error("branch journal budget exhausted: {budget}")]
    Capacity {
        /// 耗尽的预算名称。
        budget: &'static str,
    },
    /// 无运行时、工作任务失败或共享状态中毒，拒绝继续执行资源动作。
    #[error("RM coordination unavailable: {reason}")]
    Unavailable {
        /// 固定说明。
        reason: &'static str,
    },
}

impl From<std::io::Error> for RmError {
    fn from(error: std::io::Error) -> Self {
        Self::Storage { kind: error.kind() }
    }
}
