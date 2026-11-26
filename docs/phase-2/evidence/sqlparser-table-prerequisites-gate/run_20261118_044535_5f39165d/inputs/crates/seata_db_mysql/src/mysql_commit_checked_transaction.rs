use crate::{DbError, MySqlPreparedTransaction};
use seata_at::MySqlJdbcProfile;
use seata_rm::RegisteredBranch;

/// 在原业务事务中持有提交前恢复行锁的作用域；当前仍只允许整体回滚或丢弃。
/// 对应 Java: 无直接对象；ConnectionProxy 一阶段提交前仲裁的 Rust 所有权扩展。
///
/// ```compile_fail
/// async fn forbidden(value: seata_db_mysql::MySqlCommitCheckedTransaction) {
///     value.commit().await;
/// }
/// ```
#[derive(Debug)]
pub struct MySqlCommitCheckedTransaction {
    prepared: MySqlPreparedTransaction,
}
impl MySqlCommitCheckedTransaction {
    /// 参数为在原事务内完成仲裁的准备作用域；仅内部成功路径可构造。
    pub(crate) fn new(prepared: MySqlPreparedTransaction) -> Self {
        Self { prepared }
    }
    /// 无参数；返回原成功回执，无实际变化时为 None，不证明业务 COMMIT。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        self.prepared.registered_branch()
    }
    /// 无参数；返回原固定 JDBC 配置，不允许在仲裁后切换配置。
    pub const fn profile(&self) -> MySqlJdbcProfile {
        self.prepared.profile()
    }
    /// 无参数；返回原内存 Undo 项数，不代表数据库已提交行数。
    pub fn undo_log_count(&self) -> usize {
        self.prepared.undo_log_count()
    }
    /// 消耗整个仲裁作用域，回滚原业务与 Undo 并释放恢复行锁；独立归属记录保留。
    pub async fn rollback(self) -> Result<(), DbError> {
        self.prepared.rollback().await
    }
}
