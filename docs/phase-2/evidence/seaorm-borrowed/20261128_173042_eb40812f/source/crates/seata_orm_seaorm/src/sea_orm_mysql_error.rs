/// SeaORM 适配错误；保留正式 AT 错误分类，静态原因不包含原 SQL 或绑定值。
/// 对应 Java: 无直接对象；JDBC StatementProxy 输入验证的 Rust ORM 适配扩展。
#[derive(Debug, thiserror::Error)]
pub enum SeaOrmMySqlError {
    /// 不能无损适配的后端、值类型或输入形状。
    #[error("SeaORM MySQL input rejected: {reason}")]
    Input {
        /// 固定的拒绝原因，不回显输入。
        reason: &'static str,
    },
    /// 正式 SQL 识别器的已脱敏错误。
    #[error(transparent)]
    Sql(#[from] seata_at::SqlError),
    /// 正式值域的已脱敏错误。
    #[error(transparent)]
    Image(#[from] seata_datasource::ImageError),
    /// 正式本地事务返回的已脱敏执行错误。
    #[error(transparent)]
    Database(#[from] seata_db_mysql::DbError),
}
