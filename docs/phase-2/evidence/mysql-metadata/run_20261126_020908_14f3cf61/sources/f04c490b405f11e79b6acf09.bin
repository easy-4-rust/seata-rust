use crate::ProtocolError;

/// GlobalStatus协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.model.GlobalStatus
/// 状态值不能覆盖响应ResultCode；完成、失败及运维处置须结合原TC响应规则判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlobalStatus {
    /// 事务开始前或无法确定的状态。
    Unknown,
    /// 阶段一开始，可注册新分支。
    Begin,
    /// 二阶段提交正在执行。
    Committing,
    /// 提交遇到可恢复失败，TC负责重试。
    CommitRetrying,
    /// 二阶段回滚正在执行。
    Rollbacking,
    /// 回滚遇到可恢复失败，TC负责重试。
    RollbackRetrying,
    /// 事务超时触发回滚。
    TimeoutRollbacking,
    /// 超时回滚失败后重试。
    TimeoutRollbackRetrying,
    /// 所有分支可异步提交、清理尚未完成；仅在成功响应及相应上层契约下可视为已提交。
    AsyncCommitting,
    /// 全局事务提交成功。
    Committed,
    /// Java枚举列为二阶段提交失败结果；是否可人工重试由TC控制台规则决定。
    CommitFailed,
    /// 全局事务回滚成功。
    Rollbacked,
    /// Java枚举列为二阶段回滚失败结果；是否可人工重试由TC控制台规则决定。
    RollbackFailed,
    /// 超时触发的回滚成功。
    TimeoutRollbacked,
    /// 超时触发的回滚失败，仍须按TC查询/运维规则处理。
    TimeoutRollbackFailed,
    /// 会话不再受管理；不存在事务及Saga报告均可能使用，不能单据此推断提交成功。
    Finished,
    /// 提交重试达到时限，须保留该权威结果供运维处置。
    CommitRetryTimeout,
    /// 回滚重试达到时限，不能启动与TC竞争的本地重试。
    RollbackRetryTimeout,
    /// 事务正在删除。
    Deleting,
    /// 运维停止提交或提交重试。
    StopCommitOrCommitRetry,
    /// 运维停止回滚或回滚重试。
    StopRollbackOrRollbackRetry,
}

impl GlobalStatus {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Begin),
            2 => Ok(Self::Committing),
            3 => Ok(Self::CommitRetrying),
            4 => Ok(Self::Rollbacking),
            5 => Ok(Self::RollbackRetrying),
            6 => Ok(Self::TimeoutRollbacking),
            7 => Ok(Self::TimeoutRollbackRetrying),
            8 => Ok(Self::AsyncCommitting),
            9 => Ok(Self::Committed),
            10 => Ok(Self::CommitFailed),
            11 => Ok(Self::Rollbacked),
            12 => Ok(Self::RollbackFailed),
            13 => Ok(Self::TimeoutRollbacked),
            14 => Ok(Self::TimeoutRollbackFailed),
            15 => Ok(Self::Finished),
            16 => Ok(Self::CommitRetryTimeout),
            17 => Ok(Self::RollbackRetryTimeout),
            18 => Ok(Self::Deleting),
            19 => Ok(Self::StopCommitOrCommitRetry),
            20 => Ok(Self::StopRollbackOrRollbackRetry),
            _ => Err(ProtocolError::UnknownCode {
                kind: "GlobalStatus",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Begin => 1,
            Self::Committing => 2,
            Self::CommitRetrying => 3,
            Self::Rollbacking => 4,
            Self::RollbackRetrying => 5,
            Self::TimeoutRollbacking => 6,
            Self::TimeoutRollbackRetrying => 7,
            Self::AsyncCommitting => 8,
            Self::Committed => 9,
            Self::CommitFailed => 10,
            Self::Rollbacked => 11,
            Self::RollbackFailed => 12,
            Self::TimeoutRollbacked => 13,
            Self::TimeoutRollbackFailed => 14,
            Self::Finished => 15,
            Self::CommitRetryTimeout => 16,
            Self::RollbackRetryTimeout => 17,
            Self::Deleting => 18,
            Self::StopCommitOrCommitRetry => 19,
            Self::StopRollbackOrRollbackRetry => 20,
        }
    }
}
