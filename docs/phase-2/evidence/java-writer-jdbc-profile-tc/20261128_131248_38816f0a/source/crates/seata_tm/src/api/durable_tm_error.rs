use crate::{TmError, Xid};

/// 来源或发送门禁失败时保留 TC XID，但绝不暗示该操作已发送。
/// 对应 Java: DefaultGlobalTransaction 的事务异常边界。
#[derive(Debug, thiserror::Error)]
pub enum DurableTmError {
    /// TM 原请求失败，保留明确拒绝与未知结果分类。
    #[error(transparent)]
    Tm(#[from] TmError),
    /// 持久存储未确认，调用方必须停止本地业务或结束发送。
    #[error("TM durable {stage} not confirmed for {xid}")]
    Persistence {
        /// 未确认的持久写入阶段。
        stage: &'static str,
        /// TC 返回的原 XID。
        xid: Xid,
        /// 脱敏的持久层错误。
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
