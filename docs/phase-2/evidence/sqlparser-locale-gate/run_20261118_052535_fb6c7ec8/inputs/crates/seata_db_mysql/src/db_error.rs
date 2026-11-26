use seata_datasource::{ImageError, MetadataError};

/// MySQL基础错误，只保留SQLSTATE/编号及静态阶段，不公开SQLx错误中的参数或凭据。
/// 对应 Java: java.sql.SQLException与TableMetaException的Rust类型化边界。
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// 原回调已经绑定分支；其未知锁键不能补造成一阶段成功登记。
    #[error("TC callback already owns this branch; phase-one enrollment cannot continue")]
    CallbackObserved,
    /// 原身份、JDBC 配置或二阶段方向与独立恢复行冲突，禁止执行资源动作。
    #[error("MySQL RM recovery identity or phase conflict")]
    RecoveryConflict,
    /// 本地请求无效，未携带原SQL或参数值。
    #[error("invalid MySQL request: {reason}")]
    Input {
        /// 固定校验原因。
        reason: &'static str,
    },
    /// 精确镜像结构或预算校验错误。
    #[error(transparent)]
    Image(#[from] ImageError),
    /// 本地配置无效。
    #[error("invalid MySQL configuration: {reason}")]
    Invalid {
        /// 固定校验原因。
        reason: &'static str,
    },
    /// 当前实现不能证明该数据库形状安全。
    #[error("unsupported MySQL AT feature: {feature}")]
    Unsupported {
        /// 固定功能名称。
        feature: &'static str,
    },
    /// 真实驱动错误；消息不携带原始SQL或服务器错误文本。
    #[error("MySQL operation {operation} failed (state={sql_state:?}, code={code:?})")]
    Driver {
        /// 固定调用阶段。
        operation: &'static str,
        /// SQLSTATE（如果服务端提供）。
        sql_state: Option<String>,
        /// MySQL原始错误编号。
        code: Option<u16>,
    },
    /// 本地元数据结构校验错误。
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    /// 原Undo编码结构/预算不满足要求，不携带原业务值。
    #[error(transparent)]
    UndoCodec(#[from] seata_at::UndoCodecError),
    /// 已写入Undo的事务禁止继续变更，避免业务与准备日志脱节。
    #[error("local MySQL transaction Undo is sealed; rollback required")]
    UndoSealed,
    /// 锁定当前数据与原前后镜像均不一致；日志保留，禁止局部补偿提交。
    #[error("dirty MySQL records during Undo; original log retained")]
    UndoDirty,
    /// 操作失败/取消后的同一本地作用域不可继续执行。
    #[error("local MySQL transaction is poisoned; rollback/drop required")]
    Poisoned,
    /// 本地作用域已关闭。
    #[error("local MySQL transaction is closed")]
    Closed,
    /// 关闭等待超限，不表示所有在途资源均已清理。
    #[error("MySQL operation deadline exceeded: {operation}")]
    Deadline {
        /// 固定调用阶段。
        operation: &'static str,
    },
}
impl DbError {
    /// 参数为原驱动错误与固定阶段；返回脱敏SQLSTATE/编号，原始服务器文本不进入公开错误。
    pub(crate) fn driver(operation: &'static str, source: sqlx::Error) -> Self {
        let mysql = source
            .as_database_error()
            .and_then(|error| error.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>());
        Self::Driver {
            operation,
            sql_state: mysql.and_then(|error| error.code().map(str::to_owned)),
            code: mysql.map(sqlx::mysql::MySqlDatabaseError::number),
        }
    }
}
