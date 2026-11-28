use crate::ProtocolError;

/// MessageType协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.protocol.MessageType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    /// GlobalBeginRequest消息。
    GlobalBeginRequest,
    /// GlobalBeginResponse消息。
    GlobalBeginResponse,
    /// BranchCommitRequest消息。
    BranchCommitRequest,
    /// BranchCommitResponse消息。
    BranchCommitResponse,
    /// BranchRollbackRequest消息。
    BranchRollbackRequest,
    /// BranchRollbackResponse消息。
    BranchRollbackResponse,
    /// GlobalCommitRequest消息。
    GlobalCommitRequest,
    /// GlobalCommitResponse消息。
    GlobalCommitResponse,
    /// GlobalRollbackRequest消息。
    GlobalRollbackRequest,
    /// GlobalRollbackResponse消息。
    GlobalRollbackResponse,
    /// BranchRegisterRequest消息。
    BranchRegisterRequest,
    /// BranchRegisterResponse消息。
    BranchRegisterResponse,
    /// BranchReportRequest消息。
    BranchReportRequest,
    /// BranchReportResponse消息。
    BranchReportResponse,
    /// GlobalStatusRequest消息。
    GlobalStatusRequest,
    /// GlobalStatusResponse消息。
    GlobalStatusResponse,
    /// GlobalReportRequest消息。
    GlobalReportRequest,
    /// GlobalReportResponse消息。
    GlobalReportResponse,
    /// GlobalLockQueryRequest消息。
    GlobalLockQueryRequest,
    /// GlobalLockQueryResponse消息。
    GlobalLockQueryResponse,
    /// MergedWarp消息。
    MergedWarp,
    /// MergeResult消息。
    MergeResult,
    /// RegisterTmRequest消息。
    RegisterTmRequest,
    /// RegisterTmResponse消息。
    RegisterTmResponse,
    /// RegisterRmRequest消息。
    RegisterRmRequest,
    /// RegisterRmResponse消息。
    RegisterRmResponse,
    /// UnregisterRmRequest消息。
    UnregisterRmRequest,
    /// UnregisterRmResponse消息。
    UnregisterRmResponse,
    /// UndoLogDeleteRequest消息。
    UndoLogDeleteRequest,
    /// Heartbeat消息。
    Heartbeat,
    /// BatchResult消息。
    BatchResult,
}

impl MessageType {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u16) -> Result<Self, ProtocolError> {
        match code {
            1 => Ok(Self::GlobalBeginRequest),
            2 => Ok(Self::GlobalBeginResponse),
            3 => Ok(Self::BranchCommitRequest),
            4 => Ok(Self::BranchCommitResponse),
            5 => Ok(Self::BranchRollbackRequest),
            6 => Ok(Self::BranchRollbackResponse),
            7 => Ok(Self::GlobalCommitRequest),
            8 => Ok(Self::GlobalCommitResponse),
            9 => Ok(Self::GlobalRollbackRequest),
            10 => Ok(Self::GlobalRollbackResponse),
            11 => Ok(Self::BranchRegisterRequest),
            12 => Ok(Self::BranchRegisterResponse),
            13 => Ok(Self::BranchReportRequest),
            14 => Ok(Self::BranchReportResponse),
            15 => Ok(Self::GlobalStatusRequest),
            16 => Ok(Self::GlobalStatusResponse),
            17 => Ok(Self::GlobalReportRequest),
            18 => Ok(Self::GlobalReportResponse),
            21 => Ok(Self::GlobalLockQueryRequest),
            22 => Ok(Self::GlobalLockQueryResponse),
            59 => Ok(Self::MergedWarp),
            60 => Ok(Self::MergeResult),
            101 => Ok(Self::RegisterTmRequest),
            102 => Ok(Self::RegisterTmResponse),
            103 => Ok(Self::RegisterRmRequest),
            104 => Ok(Self::RegisterRmResponse),
            105 => Ok(Self::UnregisterRmRequest),
            106 => Ok(Self::UnregisterRmResponse),
            111 => Ok(Self::UndoLogDeleteRequest),
            120 => Ok(Self::Heartbeat),
            121 => Ok(Self::BatchResult),
            _ => Err(ProtocolError::UnknownCode {
                kind: "MessageType",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u16 {
        match self {
            Self::GlobalBeginRequest => 1,
            Self::GlobalBeginResponse => 2,
            Self::BranchCommitRequest => 3,
            Self::BranchCommitResponse => 4,
            Self::BranchRollbackRequest => 5,
            Self::BranchRollbackResponse => 6,
            Self::GlobalCommitRequest => 7,
            Self::GlobalCommitResponse => 8,
            Self::GlobalRollbackRequest => 9,
            Self::GlobalRollbackResponse => 10,
            Self::BranchRegisterRequest => 11,
            Self::BranchRegisterResponse => 12,
            Self::BranchReportRequest => 13,
            Self::BranchReportResponse => 14,
            Self::GlobalStatusRequest => 15,
            Self::GlobalStatusResponse => 16,
            Self::GlobalReportRequest => 17,
            Self::GlobalReportResponse => 18,
            Self::GlobalLockQueryRequest => 21,
            Self::GlobalLockQueryResponse => 22,
            Self::MergedWarp => 59,
            Self::MergeResult => 60,
            Self::RegisterTmRequest => 101,
            Self::RegisterTmResponse => 102,
            Self::RegisterRmRequest => 103,
            Self::RegisterRmResponse => 104,
            Self::UnregisterRmRequest => 105,
            Self::UnregisterRmResponse => 106,
            Self::UndoLogDeleteRequest => 111,
            Self::Heartbeat => 120,
            Self::BatchResult => 121,
        }
    }

    /// 返回该类型是否在已批准的V1载荷allowlist内；不表示其codec已经完成，心跳仅用外层空帧。
    pub const fn in_v1_payload_allowlist(self) -> bool {
        !matches!(
            self,
            Self::UnregisterRmRequest
                | Self::UnregisterRmResponse
                | Self::Heartbeat
                | Self::BatchResult
        )
    }
}
