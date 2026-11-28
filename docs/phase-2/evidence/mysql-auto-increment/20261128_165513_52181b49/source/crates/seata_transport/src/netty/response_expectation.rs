use crate::TransportError;
use seata_protocol::{Message, MessageType, PayloadLimits, ProtocolError};

/// 等待项的完整响应类型/平面合并形状；不能只凭请求ID完成不匹配的消息。
/// 对应 Java: MessageFuture 的响应关联职责；补充显式类型守卫。
#[derive(Debug)]
pub(crate) enum ResponseExpectation {
    Single(MessageType),
    Batch(Vec<MessageType>),
}
impl ResponseExpectation {
    pub(crate) fn from_request(
        value: &Message,
        limits: PayloadLimits,
    ) -> Result<Self, TransportError> {
        if let Message::MergedWarpMessage(batch) = value {
            if batch.msgs.len() > limits.maximum_batch_children()
                || batch.msgs.len().saturating_add(1) > limits.maximum_total_nodes()
                || limits.maximum_nesting_depth() == 0
            {
                return Err(TransportError::Protocol(ProtocolError::LimitExceeded {
                    limit: "request batch nodes",
                    maximum: limits.maximum_total_nodes(),
                    actual: batch.msgs.len().saturating_add(1),
                }));
            }
            let mut expected = Vec::new();
            for child in &batch.msgs {
                expected.push(Self::ordinary(child)?);
            }
            Ok(Self::Batch(expected))
        } else {
            Ok(Self::Single(Self::ordinary(value)?))
        }
    }
    fn ordinary(value: &Message) -> Result<MessageType, TransportError> {
        Ok(match value {
            Message::GlobalBeginRequest(_) => MessageType::GlobalBeginResponse,
            Message::GlobalCommitRequest(_) => MessageType::GlobalCommitResponse,
            Message::GlobalRollbackRequest(_) => MessageType::GlobalRollbackResponse,
            Message::GlobalStatusRequest(_) => MessageType::GlobalStatusResponse,
            Message::GlobalReportRequest(_) => MessageType::GlobalReportResponse,
            Message::BranchCommitRequest(_) => MessageType::BranchCommitResponse,
            Message::BranchRollbackRequest(_) => MessageType::BranchRollbackResponse,
            Message::BranchRegisterRequest(_) => MessageType::BranchRegisterResponse,
            Message::BranchReportRequest(_) => MessageType::BranchReportResponse,
            Message::GlobalLockQueryRequest(_) => MessageType::GlobalLockQueryResponse,
            Message::RegisterTMRequest(_) => MessageType::RegisterTmResponse,
            Message::RegisterRMRequest(_) => MessageType::RegisterRmResponse,
            _ => {
                return Err(TransportError::InvalidConfiguration {
                    reason: "correlated calls require ordinary requests or one flat request batch",
                })
            }
        })
    }
    pub(crate) fn matches(&self, value: &Message) -> bool {
        match (self, value) {
            (Self::Single(expected), actual) => *expected == actual.message_type(),
            (Self::Batch(expected), Message::MergeResultMessage(actual)) => {
                actual.msgs.as_ref().is_some_and(|children| {
                    children.len() == expected.len()
                        && children
                            .iter()
                            .zip(expected)
                            .all(|(child, kind)| child.message_type() == *kind)
                })
            }
            _ => false,
        }
    }
    /// 参数为已校验响应族的原请求/回包；返回branch-end身份是否逐项原样保留，平面批次不只检查首项。
    pub(crate) fn reply_identity_matches(request: &Message, response: &Message) -> bool {
        match (request, response) {
            (Message::BranchCommitRequest(request), Message::BranchCommitResponse(response)) => {
                response.xid.as_deref() == Some(request.xid.as_str())
                    && response.branch_id == request.branch_id
            }
            (
                Message::BranchRollbackRequest(request),
                Message::BranchRollbackResponse(response),
            ) => {
                response.xid.as_deref() == Some(request.xid.as_str())
                    && response.branch_id == request.branch_id
            }
            (Message::MergedWarpMessage(request), Message::MergeResultMessage(response)) => {
                response.msgs.as_ref().is_some_and(|replies| {
                    request.msgs.len() == replies.len()
                        && request.msgs.iter().zip(replies).all(|(request, response)| {
                            Self::reply_identity_matches(request, response)
                        })
                })
            }
            _ => true,
        }
    }
}
