use crate::DbError;
use seata_rm::RegisteredBranch;

/// 原事务提交前仲裁失败；保留真实回执，失败作用域已消耗，不表示业务已提交。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 的提交前失败边界；Rust 恢复行仲裁扩展。
#[derive(Debug, thiserror::Error)]
#[error("MySQL commit precondition check failed: {source}")]
pub struct MySqlCommitCheckError {
    receipt: Option<Box<RegisteredBranch>>,
    #[source]
    source: DbError,
}
impl MySqlCommitCheckError {
    /// 参数为真实回执与脱敏数据库错误；仅内部原作用域失败后构造，不合成注册结果。
    pub(crate) fn new(receipt: Option<RegisteredBranch>, source: DbError) -> Self {
        Self {
            receipt: receipt.map(Box::new),
            source,
        }
    }
    /// 无参数；返回已真实取得的成功回执；None 只用于无实际变更作用域。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        self.receipt.as_deref()
    }
    /// 无参数；返回原脱敏错误，供调用方区分期限、方向冲突及 schema 拒绝。
    pub fn database_error(&self) -> &DbError {
        &self.source
    }
}
