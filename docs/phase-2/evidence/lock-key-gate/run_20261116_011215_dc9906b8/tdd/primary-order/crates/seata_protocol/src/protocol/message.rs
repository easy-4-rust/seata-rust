use crate::codec::{WireReader, WireWriter};
use crate::protocol::{
    MergeResultMessage, MergedWarpMessage, RegisterRMRequest, RegisterRMResponse,
    RegisterTMRequest, RegisterTMResponse,
};
use crate::transaction::{
    AbstractTransactionResponse, BranchCommitRequest, BranchCommitResponse, BranchRegisterRequest,
    BranchRegisterResponse, BranchReportRequest, BranchReportResponse, BranchRollbackRequest,
    BranchRollbackResponse, GlobalBeginRequest, GlobalBeginResponse, GlobalCommitRequest,
    GlobalCommitResponse, GlobalLockQueryRequest, GlobalLockQueryResponse, GlobalReportRequest,
    GlobalReportResponse, GlobalRollbackRequest, GlobalRollbackResponse, GlobalStatusRequest,
    GlobalStatusResponse, UndoLogDeleteRequest,
};
use crate::{MessageType, ProtocolError};

/// 原生V1载荷的显式类型和字段；不缓存输入字节，聚合对象递归保留消息结构。
/// 对应 Java: org.apache.seata.core.protocol.AbstractMessage / SeataSerializerV1类型分派。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// 有序聚合消息及可选子请求编号。
    MergedWarpMessage(MergedWarpMessage),
    /// 有序聚合结果；子类型在编解码时严格校验。
    MergeResultMessage(MergeResultMessage),
    /// GlobalBeginRequest字段模型。
    GlobalBeginRequest(GlobalBeginRequest),
    /// GlobalBeginResponse字段模型。
    GlobalBeginResponse(GlobalBeginResponse),
    /// BranchCommitRequest字段模型。
    BranchCommitRequest(BranchCommitRequest),
    /// BranchCommitResponse字段模型。
    BranchCommitResponse(BranchCommitResponse),
    /// BranchRollbackRequest字段模型。
    BranchRollbackRequest(BranchRollbackRequest),
    /// BranchRollbackResponse字段模型。
    BranchRollbackResponse(BranchRollbackResponse),
    /// GlobalCommitRequest字段模型。
    GlobalCommitRequest(GlobalCommitRequest),
    /// GlobalCommitResponse字段模型。
    GlobalCommitResponse(GlobalCommitResponse),
    /// GlobalRollbackRequest字段模型。
    GlobalRollbackRequest(GlobalRollbackRequest),
    /// GlobalRollbackResponse字段模型。
    GlobalRollbackResponse(GlobalRollbackResponse),
    /// BranchRegisterRequest字段模型。
    BranchRegisterRequest(BranchRegisterRequest),
    /// BranchRegisterResponse字段模型。
    BranchRegisterResponse(BranchRegisterResponse),
    /// BranchReportRequest字段模型。
    BranchReportRequest(BranchReportRequest),
    /// BranchReportResponse字段模型。
    BranchReportResponse(BranchReportResponse),
    /// GlobalStatusRequest字段模型。
    GlobalStatusRequest(GlobalStatusRequest),
    /// GlobalStatusResponse字段模型。
    GlobalStatusResponse(GlobalStatusResponse),
    /// GlobalReportRequest字段模型。
    GlobalReportRequest(GlobalReportRequest),
    /// GlobalReportResponse字段模型。
    GlobalReportResponse(GlobalReportResponse),
    /// GlobalLockQueryRequest字段模型。
    GlobalLockQueryRequest(GlobalLockQueryRequest),
    /// GlobalLockQueryResponse字段模型。
    GlobalLockQueryResponse(GlobalLockQueryResponse),
    /// RegisterTMRequest字段模型。
    RegisterTMRequest(RegisterTMRequest),
    /// RegisterTMResponse字段模型。
    RegisterTMResponse(RegisterTMResponse),
    /// RegisterRMRequest字段模型。
    RegisterRMRequest(RegisterRMRequest),
    /// RegisterRMResponse字段模型。
    RegisterRMResponse(RegisterRMResponse),
    /// UndoLogDeleteRequest字段模型。
    UndoLogDeleteRequest(UndoLogDeleteRequest),
}

impl Message {
    /// 返回该对象的Java消息种类，不依赖枚举ordinal。
    pub const fn message_type(&self) -> MessageType {
        match self {
            Self::MergedWarpMessage(_) => MessageType::MergedWarp,
            Self::MergeResultMessage(_) => MessageType::MergeResult,
            Self::GlobalBeginRequest(_) => MessageType::GlobalBeginRequest,
            Self::GlobalBeginResponse(_) => MessageType::GlobalBeginResponse,
            Self::BranchCommitRequest(_) => MessageType::BranchCommitRequest,
            Self::BranchCommitResponse(_) => MessageType::BranchCommitResponse,
            Self::BranchRollbackRequest(_) => MessageType::BranchRollbackRequest,
            Self::BranchRollbackResponse(_) => MessageType::BranchRollbackResponse,
            Self::GlobalCommitRequest(_) => MessageType::GlobalCommitRequest,
            Self::GlobalCommitResponse(_) => MessageType::GlobalCommitResponse,
            Self::GlobalRollbackRequest(_) => MessageType::GlobalRollbackRequest,
            Self::GlobalRollbackResponse(_) => MessageType::GlobalRollbackResponse,
            Self::BranchRegisterRequest(_) => MessageType::BranchRegisterRequest,
            Self::BranchRegisterResponse(_) => MessageType::BranchRegisterResponse,
            Self::BranchReportRequest(_) => MessageType::BranchReportRequest,
            Self::BranchReportResponse(_) => MessageType::BranchReportResponse,
            Self::GlobalStatusRequest(_) => MessageType::GlobalStatusRequest,
            Self::GlobalStatusResponse(_) => MessageType::GlobalStatusResponse,
            Self::GlobalReportRequest(_) => MessageType::GlobalReportRequest,
            Self::GlobalReportResponse(_) => MessageType::GlobalReportResponse,
            Self::GlobalLockQueryRequest(_) => MessageType::GlobalLockQueryRequest,
            Self::GlobalLockQueryResponse(_) => MessageType::GlobalLockQueryResponse,
            Self::RegisterTMRequest(_) => MessageType::RegisterTmRequest,
            Self::RegisterTMResponse(_) => MessageType::RegisterTmResponse,
            Self::RegisterRMRequest(_) => MessageType::RegisterRmRequest,
            Self::RegisterRMResponse(_) => MessageType::RegisterRmResponse,
            Self::UndoLogDeleteRequest(_) => MessageType::UndoLogDeleteRequest,
        }
    }

    /// 返回普通事务响应前缀；身份响应和请求返回None，不伪造普通ResultCode。
    pub const fn transaction_response(&self) -> Option<&AbstractTransactionResponse> {
        match self {
            Self::GlobalBeginResponse(value) => Some(&value.response),
            Self::BranchCommitResponse(value) => Some(&value.response),
            Self::BranchRollbackResponse(value) => Some(&value.response),
            Self::GlobalCommitResponse(value) => Some(&value.response),
            Self::GlobalRollbackResponse(value) => Some(&value.response),
            Self::BranchRegisterResponse(value) => Some(&value.response),
            Self::BranchReportResponse(value) => Some(&value.response),
            Self::GlobalStatusResponse(value) => Some(&value.response),
            Self::GlobalReportResponse(value) => Some(&value.response),
            Self::GlobalLockQueryResponse(value) => Some(&value.response),
            _ => None,
        }
    }

    /// 参数为种类和已消费类型前缀的游标；未实现类型明确拒绝。
    pub(crate) fn read(
        message_type: MessageType,
        reader: &mut WireReader<'_>,
    ) -> Result<Self, ProtocolError> {
        match message_type {
            MessageType::GlobalBeginRequest => {
                Ok(Self::GlobalBeginRequest(GlobalBeginRequest::read(reader)?))
            }
            MessageType::GlobalBeginResponse => Ok(Self::GlobalBeginResponse(
                GlobalBeginResponse::read(reader)?,
            )),
            MessageType::BranchCommitRequest => Ok(Self::BranchCommitRequest(
                BranchCommitRequest::read(reader)?,
            )),
            MessageType::BranchCommitResponse => Ok(Self::BranchCommitResponse(
                BranchCommitResponse::read(reader)?,
            )),
            MessageType::BranchRollbackRequest => Ok(Self::BranchRollbackRequest(
                BranchRollbackRequest::read(reader)?,
            )),
            MessageType::BranchRollbackResponse => Ok(Self::BranchRollbackResponse(
                BranchRollbackResponse::read(reader)?,
            )),
            MessageType::GlobalCommitRequest => Ok(Self::GlobalCommitRequest(
                GlobalCommitRequest::read(reader)?,
            )),
            MessageType::GlobalCommitResponse => Ok(Self::GlobalCommitResponse(
                GlobalCommitResponse::read(reader)?,
            )),
            MessageType::GlobalRollbackRequest => Ok(Self::GlobalRollbackRequest(
                GlobalRollbackRequest::read(reader)?,
            )),
            MessageType::GlobalRollbackResponse => Ok(Self::GlobalRollbackResponse(
                GlobalRollbackResponse::read(reader)?,
            )),
            MessageType::BranchRegisterRequest => Ok(Self::BranchRegisterRequest(
                BranchRegisterRequest::read(reader)?,
            )),
            MessageType::BranchRegisterResponse => Ok(Self::BranchRegisterResponse(
                BranchRegisterResponse::read(reader)?,
            )),
            MessageType::BranchReportRequest => Ok(Self::BranchReportRequest(
                BranchReportRequest::read(reader)?,
            )),
            MessageType::BranchReportResponse => Ok(Self::BranchReportResponse(
                BranchReportResponse::read(reader)?,
            )),
            MessageType::GlobalStatusRequest => Ok(Self::GlobalStatusRequest(
                GlobalStatusRequest::read(reader)?,
            )),
            MessageType::GlobalStatusResponse => Ok(Self::GlobalStatusResponse(
                GlobalStatusResponse::read(reader)?,
            )),
            MessageType::GlobalReportRequest => Ok(Self::GlobalReportRequest(
                GlobalReportRequest::read(reader)?,
            )),
            MessageType::GlobalReportResponse => Ok(Self::GlobalReportResponse(
                GlobalReportResponse::read(reader)?,
            )),
            MessageType::GlobalLockQueryRequest => Ok(Self::GlobalLockQueryRequest(
                GlobalLockQueryRequest::read(reader)?,
            )),
            MessageType::GlobalLockQueryResponse => Ok(Self::GlobalLockQueryResponse(
                GlobalLockQueryResponse::read(reader)?,
            )),
            MessageType::RegisterTmRequest => {
                Ok(Self::RegisterTMRequest(RegisterTMRequest::read(reader)?))
            }
            MessageType::RegisterTmResponse => {
                Ok(Self::RegisterTMResponse(RegisterTMResponse::read(reader)?))
            }
            MessageType::RegisterRmRequest => {
                Ok(Self::RegisterRMRequest(RegisterRMRequest::read(reader)?))
            }
            MessageType::RegisterRmResponse => {
                Ok(Self::RegisterRMResponse(RegisterRMResponse::read(reader)?))
            }
            MessageType::UndoLogDeleteRequest => Ok(Self::UndoLogDeleteRequest(
                UndoLogDeleteRequest::read(reader)?,
            )),
            other => Err(ProtocolError::UnsupportedMessage {
                code: other.to_java_v1(),
            }),
        }
    }

    /// 按实际对象调用对应字段编码，不在此层添加消息类型前缀。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        match self {
            Self::MergedWarpMessage(_) | Self::MergeResultMessage(_) => {
                Err(ProtocolError::InvalidValue {
                    field: "merge",
                    reason: "aggregate requires bounded recursive codec",
                })
            }
            Self::GlobalBeginRequest(value) => value.write(writer),
            Self::GlobalBeginResponse(value) => value.write(writer),
            Self::BranchCommitRequest(value) => value.write(writer),
            Self::BranchCommitResponse(value) => value.write(writer),
            Self::BranchRollbackRequest(value) => value.write(writer),
            Self::BranchRollbackResponse(value) => value.write(writer),
            Self::GlobalCommitRequest(value) => value.write(writer),
            Self::GlobalCommitResponse(value) => value.write(writer),
            Self::GlobalRollbackRequest(value) => value.write(writer),
            Self::GlobalRollbackResponse(value) => value.write(writer),
            Self::BranchRegisterRequest(value) => value.write(writer),
            Self::BranchRegisterResponse(value) => value.write(writer),
            Self::BranchReportRequest(value) => value.write(writer),
            Self::BranchReportResponse(value) => value.write(writer),
            Self::GlobalStatusRequest(value) => value.write(writer),
            Self::GlobalStatusResponse(value) => value.write(writer),
            Self::GlobalReportRequest(value) => value.write(writer),
            Self::GlobalReportResponse(value) => value.write(writer),
            Self::GlobalLockQueryRequest(value) => value.write(writer),
            Self::GlobalLockQueryResponse(value) => value.write(writer),
            Self::RegisterTMRequest(value) => value.write(writer),
            Self::RegisterTMResponse(value) => value.write(writer),
            Self::RegisterRMRequest(value) => value.write(writer),
            Self::RegisterRMResponse(value) => value.write(writer),
            Self::UndoLogDeleteRequest(value) => value.write(writer),
        }
    }

    /// 返回是否为Java AbstractResultMessage的已支持子类；聚合容器自身不是结果子类。
    pub const fn is_result(&self) -> bool {
        matches!(
            self,
            Self::GlobalBeginResponse(_)
                | Self::GlobalCommitResponse(_)
                | Self::GlobalRollbackResponse(_)
                | Self::GlobalStatusResponse(_)
                | Self::GlobalReportResponse(_)
                | Self::GlobalLockQueryResponse(_)
                | Self::BranchRegisterResponse(_)
                | Self::BranchReportResponse(_)
                | Self::BranchCommitResponse(_)
                | Self::BranchRollbackResponse(_)
                | Self::RegisterTMResponse(_)
                | Self::RegisterRMResponse(_)
        )
    }
}
