use crate::RmOperation;
use seata_protocol::{protocol::RpcMessage, Message};
use seata_transport::{CallError, TransportError};

/// 单次RM调用错误；保留完整原请求、TC响应及传输身份，Debug沿用协议字段脱敏。
/// 对应 Java: RmTransactionException；按ADR-0008区分明确拒绝和未知结果。
#[derive(Debug, thiserror::Error)]
pub enum RmCallError {
    /// 高层输入校验失败，尚未发送业务帧。
    #[error("invalid RM call: {reason}")]
    Invalid {
        /// 固定诊断，不插入业务数据。
        reason: &'static str,
    },
    /// TC实际返回Failed，保留所有响应字段；不能推断完全没有副作用。
    #[error("TC rejected RM {operation:?}")]
    DefiniteRejected {
        /// 原操作。
        operation: RmOperation,
        /// 完整原始请求。
        request: Box<Message>,
        /// 原始TC响应，包括Failed前缀及返回分支号/锁结果。
        response: Box<RpcMessage>,
    },
    /// 明确未开始发送此业务帧。
    #[error("RM {operation:?} was not sent: {source}")]
    LocalFailureBeforeSend {
        /// 原操作。
        operation: RmOperation,
        /// 完整原始请求。
        request: Box<Message>,
        /// 原始传输原因及身份。
        source: Box<CallError>,
    },
    /// 已可能发送，没有权威结果；不自动重发。
    #[error("RM {operation:?} outcome unknown: {source}")]
    UnknownOutcome {
        /// 原操作。
        operation: RmOperation,
        /// 完整原始请求，不含虚构分支号。
        request: Box<Message>,
        /// 原始传输原因及身份。
        source: Box<CallError>,
    },
    /// 协议/语义响应不能接受；是否可能已发送独立保留。
    #[error("RM {operation:?} protocol failure: {reason}")]
    ProtocolFailure {
        /// 原操作。
        operation: RmOperation,
        /// 完整原始请求。
        request: Box<Message>,
        /// 已解码时保留完整响应。
        response: Option<Box<RpcMessage>>,
        /// 是否已可能发送；true须按未知结果处理。
        may_have_effect: bool,
        /// 固定诊断。
        reason: &'static str,
        /// 原传输协议错误；语义失败时没有该值。
        source: Option<Box<CallError>>,
    },
}
impl RmCallError {
    /// 返回是否缺少权威结果且请求可能已经发送；不授权自动重放。
    pub const fn is_unknown_outcome(&self) -> bool {
        matches!(
            self,
            Self::UnknownOutcome { .. }
                | Self::ProtocolFailure {
                    may_have_effect: true,
                    ..
                }
        )
    }
    /// 将参数原操作、原请求及传输失败映射为明确未发送、未知或协议失败。
    pub(crate) fn from_call(operation: RmOperation, request: Message, source: CallError) -> Self {
        let may_have_effect = false;
        let protocol = match &source {
            CallError::LocalFailureBeforeSend { reason, .. }
            | CallError::UnknownOutcome { reason, .. } => matches!(
                reason,
                TransportError::Protocol(_)
                    | TransportError::UnexpectedResponse { .. }
                    | TransportError::UnexpectedEof { .. }
                    | TransportError::DecoderClosed
            ),
            _ => true,
        };
        if protocol {
            Self::ProtocolFailure {
                operation,
                request: Box::new(request),
                response: None,
                may_have_effect,
                reason: "unacceptable wire response",
                source: Some(Box::new(source)),
            }
        } else if may_have_effect {
            Self::UnknownOutcome {
                operation,
                request: Box::new(request),
                source: Box::new(source),
            }
        } else {
            Self::LocalFailureBeforeSend {
                operation,
                request: Box::new(request),
                source: Box::new(source),
            }
        }
    }
}
