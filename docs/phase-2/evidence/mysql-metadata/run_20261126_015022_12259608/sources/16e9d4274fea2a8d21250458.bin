use crate::ProtocolError;

/// BranchStatus协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.model.BranchStatus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchStatus {
    /// 未知业务状态值，仍须区分非法线缆码。
    Unknown,
    /// 分支已登记。
    Registered,
    /// 一阶段完成。
    PhaseOneDone,
    /// 一阶段失败。
    PhaseOneFailed,
    /// 一阶段超时。
    PhaseOneTimeout,
    /// 二阶段提交完成。
    PhaseTwoCommitted,
    /// 二阶段提交失败，可重试。
    PhaseTwoCommitFailedRetryable,
    /// 二阶段提交失败，不可重试。
    PhaseTwoCommitFailedUnretryable,
    /// 二阶段回滚完成。
    PhaseTwoRollbacked,
    /// 二阶段回滚失败，可重试。
    PhaseTwoRollbackFailedRetryable,
    /// 二阶段回滚失败，不可重试。
    PhaseTwoRollbackFailedUnretryable,
    /// XA提交返回XAER_NOTA，仍可重试。
    PhaseTwoCommitFailedXaerNotaRetryable,
    /// XA回滚返回XAER_NOTA，仍可重试。
    PhaseTwoRollbackFailedXaerNotaRetryable,
    /// XA一阶段只读分支。
    PhaseOneReadOnly,
    /// 运维停止分支重试。
    StopRetry,
    /// 一阶段准备失败。
    PhaseOnePrepareFailed,
}

impl BranchStatus {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Registered),
            2 => Ok(Self::PhaseOneDone),
            3 => Ok(Self::PhaseOneFailed),
            4 => Ok(Self::PhaseOneTimeout),
            5 => Ok(Self::PhaseTwoCommitted),
            6 => Ok(Self::PhaseTwoCommitFailedRetryable),
            7 => Ok(Self::PhaseTwoCommitFailedUnretryable),
            8 => Ok(Self::PhaseTwoRollbacked),
            9 => Ok(Self::PhaseTwoRollbackFailedRetryable),
            10 => Ok(Self::PhaseTwoRollbackFailedUnretryable),
            11 => Ok(Self::PhaseTwoCommitFailedXaerNotaRetryable),
            12 => Ok(Self::PhaseTwoRollbackFailedXaerNotaRetryable),
            13 => Ok(Self::PhaseOneReadOnly),
            14 => Ok(Self::StopRetry),
            15 => Ok(Self::PhaseOnePrepareFailed),
            _ => Err(ProtocolError::UnknownCode {
                kind: "BranchStatus",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Registered => 1,
            Self::PhaseOneDone => 2,
            Self::PhaseOneFailed => 3,
            Self::PhaseOneTimeout => 4,
            Self::PhaseTwoCommitted => 5,
            Self::PhaseTwoCommitFailedRetryable => 6,
            Self::PhaseTwoCommitFailedUnretryable => 7,
            Self::PhaseTwoRollbacked => 8,
            Self::PhaseTwoRollbackFailedRetryable => 9,
            Self::PhaseTwoRollbackFailedUnretryable => 10,
            Self::PhaseTwoCommitFailedXaerNotaRetryable => 11,
            Self::PhaseTwoRollbackFailedXaerNotaRetryable => 12,
            Self::PhaseOneReadOnly => 13,
            Self::StopRetry => 14,
            Self::PhaseOnePrepareFailed => 15,
        }
    }
}
