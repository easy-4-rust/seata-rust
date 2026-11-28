use seata_at::SqlError;
use seata_datasource::ImageError;
use seata_db_mysql::DbError;
/// ORM 输入、识别和原事务错误；显示文本不包含绑定值或驱动回显。
/// 对应 Java: 无直接对象；PreparedStatementProxy 的 Rust ORM 错误适配扩展。
#[derive(thiserror::Error)]
#[non_exhaustive]
pub enum RbatisMySqlError {
    /// 输入未满足完整且有界的原值约束。
    #[error("rbatis MySQL input rejected: {reason}")]
    Input {
        /// 静态原因，不携带原值。
        reason: &'static str,
    },
    /// 原正式 SQL 识别失败。
    #[error("rbatis MySQL SQL admission rejected")]
    Sql(#[from] SqlError),
    /// 原精确值域转换失败。
    #[error("rbatis MySQL exact value rejected")]
    Image(#[from] ImageError),
    /// 原 AT 事务执行失败，保留源错误供显式检查。
    #[error("rbatis MySQL AT transaction failed")]
    Database(#[from] DbError),
}

impl std::fmt::Debug for RbatisMySqlError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_tuple("RbatisMySqlError")
            .field(&self.to_string())
            .finish()
    }
}
