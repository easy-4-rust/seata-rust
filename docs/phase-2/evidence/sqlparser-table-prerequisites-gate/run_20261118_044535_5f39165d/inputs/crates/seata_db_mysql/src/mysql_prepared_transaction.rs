use crate::{DbError, MySqlLocalTransaction};
use seata_at::MySqlJdbcProfile;
use seata_rm::RegisteredBranch;

/// 拥有尚未提交的原业务事务与真实已知回执；当前只提供回滚，不提供 COMMIT 或解封原事务。
/// 对应 Java: 无直接对象；ConnectionProxy 一阶段准备交接的 Rust 所有权扩展。
///
/// ```compile_fail
/// async fn forbidden(value: seata_db_mysql::MySqlPreparedTransaction) {
///     value.commit().await;
/// }
/// ```
#[derive(Debug)]
pub struct MySqlPreparedTransaction {
    transaction: MySqlLocalTransaction,
    receipt: Option<RegisteredBranch>,
    profile: MySqlJdbcProfile,
}
impl MySqlPreparedTransaction {
    /// 消耗原准备作用域，在原业务事务中锁定核对恢复行与 Normal Undo；返回仍只能回滚的仲裁作用域。
    /// 失败或取消关闭原事务；不授权业务提交，不证明 TC 锁覆盖或所有旧尝试均已围栏。
    pub async fn lock_commit_preconditions(
        mut self,
    ) -> Result<crate::MySqlCommitCheckedTransaction, crate::MySqlCommitCheckError> {
        if let Err(source) = self
            .transaction
            .lock_commit_preconditions(self.receipt.as_ref(), self.profile)
            .await
        {
            return Err(crate::MySqlCommitCheckError::new(
                self.receipt.take(),
                source,
            ));
        }
        Ok(crate::MySqlCommitCheckedTransaction::new(self))
    }
    /// 参数为完成准备的原事务、真实回执及固定配置；只有准备实现可以构造。
    pub(crate) fn new(
        transaction: MySqlLocalTransaction,
        receipt: Option<RegisteredBranch>,
        profile: MySqlJdbcProfile,
    ) -> Self {
        Self {
            transaction,
            receipt,
            profile,
        }
    }
    /// 无参数；返回原成功回执；无实际变化时为 None，不代表业务已经提交。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        self.receipt.as_ref()
    }
    /// 无参数；返回准备使用的固定 JDBC 配置，不允许更换配置继续作用域。
    pub const fn profile(&self) -> MySqlJdbcProfile {
        self.profile
    }
    /// 无参数；返回内存实际 Undo 项数，不是独立数据库可见行数。
    pub fn undo_log_count(&self) -> usize {
        self.transaction.undo_logs().len()
    }
    /// 消耗准备作用域，回滚原业务与 Undo；独立归属记录保留给原 TC 回调恢复。
    pub async fn rollback(self) -> Result<(), DbError> {
        self.transaction.rollback().await
    }
}
