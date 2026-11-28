use crate::DbError;
use seata_xa::{XaError, XaOperation};
/// 原生 XA 的发送边界及只读观察错误，不从缺行或 XAER_NOTA 反推终态。
/// 对应 Java: ConnectionProxyXA/ResourceManagerXA 的 SQLException/XAException 边界（部分实现）。
#[derive(Debug, thiserror::Error)]
pub enum MySqlXaError {
    /// 原身份尚不满足数据库出站约束。
    #[error(transparent)]
    Identity(#[from] XaError),
    /// 此固定操作尚未出站；不表示更早的 XA 操作或全局事务从未生效。
    #[error("XA operation {operation:?} was not sent")]
    NotSent {
        /// 固定原操作。
        operation: XaOperation,
        /// 脱敏原原因。
        #[source]
        source: DbError,
    },
    /// 此固定命令可能已经出站；不授权自动重放或宣称回滚。
    #[error("XA operation {operation:?} outcome is unknown")]
    Unknown {
        /// 固定原操作。
        operation: XaOperation,
        /// 脱敏驱动/期限原因。
        #[source]
        source: DbError,
    },
    /// 只读恢复查询失败，不代表目标 Prepared 缺失。
    #[error("XA prepared observation failed")]
    Observation {
        /// 脱敏查询原因。
        #[source]
        source: DbError,
    },
    /// 原活动作用域已失败或取消，不允许后续 DML/Prepare。
    #[error("XA active scope is poisoned")]
    Poisoned,
}
