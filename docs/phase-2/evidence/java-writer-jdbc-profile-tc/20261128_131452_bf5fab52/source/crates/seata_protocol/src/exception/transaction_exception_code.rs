use crate::ProtocolError;

/// TransactionExceptionCode协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.exception.TransactionExceptionCode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransactionExceptionCode {
    /// 未分类业务错误。
    Unknown,
    /// 开启全局事务失败。
    BeginFailed,
    /// 全局锁冲突。
    LockKeyConflict,
    /// 通信IO错误。
    Io,
    /// 分支回滚失败，可重试。
    BranchRollbackFailedRetriable,
    /// 分支回滚失败，不可重试。
    BranchRollbackFailedUnretriable,
    /// 注册分支失败。
    BranchRegisterFailed,
    /// 分支报告失败。
    BranchReportFailed,
    /// 全局锁查询失败。
    LockableCheckFailed,
    /// 分支事务不存在。
    BranchTransactionNotExist,
    /// 全局事务不存在。
    GlobalTransactionNotExist,
    /// 全局事务不活跃。
    GlobalTransactionNotActive,
    /// 全局事务状态无效。
    GlobalTransactionStatusInvalid,
    /// 发送分支提交请求失败。
    FailedToSendBranchCommitRequest,
    /// 发送分支回滚请求失败。
    FailedToSendBranchRollbackRequest,
    /// 添加分支失败。
    FailedToAddBranch,
    /// 锁定全局事务失败。
    FailedLockGlobalTransaction,
    /// 写会话失败。
    FailedWriteSession,
    /// 存储失败。
    FailedStore,
    /// 当前不是Raft领导；Java19不是Go19的fail-fast。
    NotRaftLeader,
    /// 锁冲突快速拒绝，Java码20。
    LockKeyConflictFailFast,
    /// 事务超时。
    TransactionTimeout,
    /// 提交启发式结果。
    CommitHeuristic,
    /// 事务损坏。
    Broken,
    /// 分支准备失败。
    BranchPrepareFailed,
}

impl TransactionExceptionCode {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::BeginFailed),
            2 => Ok(Self::LockKeyConflict),
            3 => Ok(Self::Io),
            4 => Ok(Self::BranchRollbackFailedRetriable),
            5 => Ok(Self::BranchRollbackFailedUnretriable),
            6 => Ok(Self::BranchRegisterFailed),
            7 => Ok(Self::BranchReportFailed),
            8 => Ok(Self::LockableCheckFailed),
            9 => Ok(Self::BranchTransactionNotExist),
            10 => Ok(Self::GlobalTransactionNotExist),
            11 => Ok(Self::GlobalTransactionNotActive),
            12 => Ok(Self::GlobalTransactionStatusInvalid),
            13 => Ok(Self::FailedToSendBranchCommitRequest),
            14 => Ok(Self::FailedToSendBranchRollbackRequest),
            15 => Ok(Self::FailedToAddBranch),
            16 => Ok(Self::FailedLockGlobalTransaction),
            17 => Ok(Self::FailedWriteSession),
            18 => Ok(Self::FailedStore),
            19 => Ok(Self::NotRaftLeader),
            20 => Ok(Self::LockKeyConflictFailFast),
            21 => Ok(Self::TransactionTimeout),
            22 => Ok(Self::CommitHeuristic),
            23 => Ok(Self::Broken),
            24 => Ok(Self::BranchPrepareFailed),
            _ => Err(ProtocolError::UnknownCode {
                kind: "TransactionExceptionCode",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::BeginFailed => 1,
            Self::LockKeyConflict => 2,
            Self::Io => 3,
            Self::BranchRollbackFailedRetriable => 4,
            Self::BranchRollbackFailedUnretriable => 5,
            Self::BranchRegisterFailed => 6,
            Self::BranchReportFailed => 7,
            Self::LockableCheckFailed => 8,
            Self::BranchTransactionNotExist => 9,
            Self::GlobalTransactionNotExist => 10,
            Self::GlobalTransactionNotActive => 11,
            Self::GlobalTransactionStatusInvalid => 12,
            Self::FailedToSendBranchCommitRequest => 13,
            Self::FailedToSendBranchRollbackRequest => 14,
            Self::FailedToAddBranch => 15,
            Self::FailedLockGlobalTransaction => 16,
            Self::FailedWriteSession => 17,
            Self::FailedStore => 18,
            Self::NotRaftLeader => 19,
            Self::LockKeyConflictFailFast => 20,
            Self::TransactionTimeout => 21,
            Self::CommitHeuristic => 22,
            Self::Broken => 23,
            Self::BranchPrepareFailed => 24,
        }
    }
}
