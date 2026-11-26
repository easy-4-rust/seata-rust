use crate::{ConnectionGeneration, TransportError};
use thiserror::Error;

/// 单次传输调用结果；没有有效TC响应时不能把写入后失败称为明确拒绝。
/// 对应 Java: MessageFuture/AbstractNettyRemoting#sendSync 的等待结果；落实ADR-0008。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CallError {
    /// 明确没有轮询此请求的首次网络写入，TC不可能因该帧产生事务效果。
    #[error("传输帧未发送（连接 {generation:?}，编号 {request_id:?}）：{reason}")]
    LocalFailureBeforeSend {
        /// 连接代际；分配代际本身失败时为None。
        generation: Option<ConnectionGeneration>,
        /// 编号尚未分配时为None；已分配编号也不会被回收。
        request_id: Option<i32>,
        /// 本地或连接错误原因。
        reason: TransportError,
    },
    /// 帧可能已经到达TC，只能由上层按事务身份查询/协调，禁止盲目重放。
    #[error("传输结果未知（连接 {generation:?}，编号 {request_id}）：{reason}")]
    UnknownOutcome {
        /// 真实发送通道的代际。
        generation: ConnectionGeneration,
        /// 此连接唯一的请求编号。
        request_id: i32,
        /// 无权威响应的原因。
        reason: TransportError,
    },
}
impl CallError {
    /// 返回此错误是否表示TC可能已处理请求；不推断提交/回滚状态。
    pub const fn is_unknown_outcome(&self) -> bool {
        matches!(self, Self::UnknownOutcome { .. })
    }
    /// 返回本调用的已分配请求编号，尚未分配时为None。
    pub const fn request_id(&self) -> Option<i32> {
        match self {
            Self::LocalFailureBeforeSend { request_id, .. } => *request_id,
            Self::UnknownOutcome { request_id, .. } => Some(*request_id),
        }
    }
    pub(crate) fn local(
        generation: Option<ConnectionGeneration>,
        request_id: Option<i32>,
        reason: TransportError,
    ) -> Self {
        Self::LocalFailureBeforeSend {
            generation,
            request_id,
            reason,
        }
    }
    pub(crate) fn request(
        generation: ConnectionGeneration,
        request_id: i32,
        attempted: bool,
        reason: TransportError,
    ) -> Self {
        if attempted {
            Self::UnknownOutcome {
                generation,
                request_id,
                reason,
            }
        } else {
            Self::local(Some(generation), Some(request_id), reason)
        }
    }
}
