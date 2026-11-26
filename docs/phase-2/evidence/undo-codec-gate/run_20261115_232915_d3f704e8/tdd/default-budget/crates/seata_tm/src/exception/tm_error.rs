use crate::{RecoveryToken, TmOperation, Xid};
use seata_protocol::transaction::{AbstractTransactionResponse, GlobalBeginResponse};
use seata_protocol::GlobalStatus;
use seata_transport::{CallError, TransportError};
use thiserror::Error;
/// TM边界校验错误；不把本地校验失败解释为TC事务拒绝。
/// 对应 Java: org.apache.seata.tm.TmTransactionException；Rust按ADR-0008细分。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TmError {
    /// 在发送Commit前发现本地业务期限已到；不表示已向TC回滚。
    #[error("事务业务期限已到")]
    TransactionDeadlineExceeded {
        /// 已建立的原始事务身份。
        xid: Xid,
    },
    /// 无法启动或关闭TM拥有的连接设施，不表示某次业务操作被TC拒绝。
    #[error("TM连接设施失败：{0}")]
    Infrastructure(#[source] Box<TransportError>),
    /// 本地角色或状态不允许此操作；不发送事务帧。
    #[error("TM操作不允许：{reason}")]
    IllegalState {
        /// 当前原XID，尚未建立事务时为None。
        xid: Option<Xid>,
        /// 不含业务身份的静态诊断。
        reason: &'static str,
    },
    /// TC明确返回Failed；仍须保留随响应返回的全局状态，不能推断TC毫无副作用。
    #[error("TC明确拒绝 {operation:?}；状态 {status:?}")]
    DefiniteRejected {
        /// 原操作，不与后续恢复动作混淆。
        operation: TmOperation,
        /// 原XID，Begin无已知身份。
        xid: Option<Xid>,
        /// 完整Failed前缀，保留消息可选性与异常码。
        response: Box<AbstractTransactionResponse>,
        /// TC随失败响应返回的权威观察状态。
        status: Option<GlobalStatus>,
        /// Begin失败时完整保留其报告字段；不是已获授权可绑定的事务身份。
        reported_begin: Option<Box<GlobalBeginResponse>>,
    },
    /// 本调用尚未轮询首次网络写入。
    #[error("TM {operation:?} 未发送：{source}")]
    LocalFailureBeforeSend {
        /// 原操作。
        operation: TmOperation,
        /// 原XID（如已知）。
        xid: Option<Xid>,
        /// 传输代际、请求号和原始原因。
        #[source]
        source: Box<CallError>,
    },
    /// 请求可能已执行，无权威响应；不自动重发或改换操作。
    #[error("TM {operation:?} 结果未知：{source}")]
    UnknownOutcome {
        /// 原操作。
        operation: TmOperation,
        /// 保留原XID；Begin丢响应时仍为None。
        xid: Option<Xid>,
        /// 包含已发送标记和原始代际/编号的错误。
        #[source]
        source: Box<CallError>,
        /// 只有实际Launcher结束调用会签发；可读诊断字段不构成恢复授权。
        recovery: Option<RecoveryToken>,
    },
    /// 无法接受协议数据，是否可能已生效独立保留，禁止当作明确TC拒绝。
    #[error("TM {operation:?} 协议失败：{reason}")]
    ProtocolFailure {
        /// 原操作。
        operation: TmOperation,
        /// 原XID（如已知）。
        xid: Option<Xid>,
        /// 是否已可能向TC写入；true仍须按未知结果协调。
        may_have_effect: bool,
        /// 静态原因。
        reason: &'static str,
        /// 传输层协议错误；语义字段校验失败时为None。
        #[source]
        source: Option<Box<CallError>>,
        /// 写后协议失败的原结束来源证明；Begin/Status没有该能力。
        recovery: Option<RecoveryToken>,
    },
    /// 配置在网络请求前即不合法。
    #[error("TM配置无效：{reason}")]
    InvalidConfiguration {
        /// 不包含业务名称或凭据的静态原因。
        reason: &'static str,
    },
    /// 外部传入身份不符合边界，原值不进入日志。
    #[error("XID无效：{reason}")]
    InvalidXid {
        /// 静态原因，不包含原XID。
        reason: &'static str,
    },
}
impl TmError {
    pub(crate) fn authorize_recovery(
        &mut self,
        authority: &std::sync::Arc<()>,
        xid: Xid,
        operation: TmOperation,
    ) {
        match self {
            Self::UnknownOutcome { recovery, .. }
            | Self::ProtocolFailure {
                recovery,
                may_have_effect: true,
                ..
            } => {
                *recovery = Some(RecoveryToken::new(
                    std::sync::Arc::clone(authority),
                    xid,
                    operation,
                ))
            }
            _ => {}
        }
    }
    /// 返回Failed Begin完整原始字段，便于诊断可选XID/extraData，不将它绑定为Launcher。
    pub fn reported_begin(&self) -> Option<&GlobalBeginResponse> {
        match self {
            Self::DefiniteRejected { reported_begin, .. } => reported_begin.as_deref(),
            _ => None,
        }
    }
    /// 返回该错误是否缺少权威结果且请求可能已生效，包括写后协议失败。
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
    /// 返回原操作关联的XID；Begin丢响应不会在此产生虚构身份。
    pub fn xid(&self) -> Option<&Xid> {
        match self {
            Self::TransactionDeadlineExceeded { xid } => Some(xid),
            Self::IllegalState { xid, .. }
            | Self::DefiniteRejected { xid, .. }
            | Self::LocalFailureBeforeSend { xid, .. }
            | Self::UnknownOutcome { xid, .. }
            | Self::ProtocolFailure { xid, .. } => xid.as_ref(),
            _ => None,
        }
    }
    pub(crate) fn from_call(operation: TmOperation, xid: Option<Xid>, source: CallError) -> Self {
        let may_have_effect = source.is_unknown_outcome();
        let protocol = match &source {
            CallError::LocalFailureBeforeSend { reason, .. }
            | CallError::UnknownOutcome { reason, .. } => matches!(
                reason,
                TransportError::Protocol(_)
                    | TransportError::UnexpectedResponse { .. }
                    | TransportError::UnexpectedEof { .. }
                    | TransportError::DecoderClosed
            ),
            _ => false,
        };
        if protocol {
            Self::ProtocolFailure {
                operation,
                xid,
                may_have_effect,
                reason: "invalid protocol response",
                source: Some(Box::new(source)),
                recovery: None,
            }
        } else if may_have_effect {
            Self::UnknownOutcome {
                operation,
                xid,
                source: Box::new(source),
                recovery: None,
            }
        } else {
            Self::LocalFailureBeforeSend {
                operation,
                xid,
                source: Box::new(source),
            }
        }
    }
}
