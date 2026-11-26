use seata_datasource::MetadataError;

/// MySQL基础错误，只保留SQLSTATE/编号及静态阶段，不公开SQLx错误中的参数或凭据。
/// 对应 Java: java.sql.SQLException与TableMetaException的Rust类型化边界。
#[derive(Debug, thiserror::Error)]
pub enum DbError {
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
