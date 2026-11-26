use crate::{DbError, MySqlDataSource, MySqlLocalTransaction};
use seata_at::MySqlJdbcProfile;
#[cfg(feature = "experimental_changed_commit")]
use seata_protocol::{transaction::BranchReportRequest, BranchStatus};
use seata_rm::RegisteredBranch;
#[cfg(feature = "experimental_changed_commit")]
use seata_rm::ResourceClient;

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
    #[cfg_attr(not(feature = "experimental_changed_commit"), allow(dead_code))]
    source: MySqlDataSource,
    receipt: Option<RegisteredBranch>,
    profile: MySqlJdbcProfile,
}
impl MySqlPreparedTransaction {
    /// 消耗原准备作用域，在原业务事务中锁定核对恢复行与 Normal Undo；返回仍只能回滚的仲裁作用域。
    /// 如果恢复表已启用 004 迁移，成功时在同一事务暂存一阶段见证。
    /// 失败或取消关闭原事务；不授权业务提交，不证明 TC 锁覆盖或所有旧尝试均已围栏。
    pub async fn lock_commit_preconditions(
        mut self,
    ) -> Result<crate::MySqlCommitCheckedTransaction, crate::MySqlCommitCheckError> {
        let witness_staged = match self
            .transaction
            .lock_commit_preconditions(self.receipt.as_ref(), self.profile)
            .await
        {
            Ok(witness_staged) => witness_staged,
            Err(source) => {
                return Err(crate::MySqlCommitCheckError::new(
                    self.receipt.take(),
                    source,
                ))
            }
        };
        Ok(crate::MySqlCommitCheckedTransaction::new(
            self,
            witness_staged,
        ))
    }
    /// 参数为完成准备的原事务、真实回执及固定配置；只有准备实现可以构造。
    pub(crate) fn new(
        transaction: MySqlLocalTransaction,
        source: MySqlDataSource,
        receipt: Option<RegisteredBranch>,
        profile: MySqlJdbcProfile,
    ) -> Self {
        Self {
            transaction,
            source,
            receipt,
            profile,
        }
    }
    /// 无参数；返回原成功回执；无实际变化时为 None，不代表业务已经提交。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        self.receipt.as_ref()
    }
    #[cfg(feature = "experimental_changed_commit")]
    /// 无参数；返回原准备时绑定的数据源，供提交前独立尝试记录使用。
    pub(crate) fn source(&self) -> &MySqlDataSource {
        &self.source
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
    #[cfg(feature = "experimental_changed_commit")]
    /// 消耗原准备事务；确认本地回滚后，对已登记分支单次报告 PhaseOne_Failed。
    /// 参数为原 RM 客户端；返回原登记回执，无分支时返回 None。回滚结果未知绝不报告。
    pub async fn rollback_and_report_failure(
        self,
        client: &ResourceClient,
    ) -> Result<Option<RegisteredBranch>, crate::MySqlPhaseOneFailureReportError> {
        let receipt = self.receipt.clone();
        self.transaction.rollback().await.map_err(|source| {
            crate::MySqlPhaseOneFailureReportError::Local {
                receipt: receipt.clone().map(Box::new),
                source,
            }
        })?;
        let Some(receipt) = receipt else {
            return Ok(None);
        };
        let identity = receipt.identity();
        // 只使用真实回执，不补造分支号；本地回滚成功前不存在任何 TC 失败报告。
        let request = BranchReportRequest {
            xid: Some(identity.xid().to_owned()),
            branch_id: identity.branch_id(),
            status: BranchStatus::PhaseOneFailed,
            resource_id: Some(identity.resource().resource_id().to_owned()),
            application_data: identity.application_data().map(str::to_owned),
            branch_type: identity.resource().branch_type(),
        };
        client.branch_report(request).await.map_err(|source| {
            crate::MySqlPhaseOneFailureReportError::Report {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            }
        })?;
        Ok(Some(receipt))
    }
    /// 仅内部已检查作用域调用；消耗无变化原事务并发送一次 COMMIT。
    pub(crate) async fn commit_no_change(self) -> Result<(), DbError> {
        self.transaction.commit_no_change().await
    }

    #[cfg(feature = "experimental_changed_commit")]
    /// 仅供已仲裁的实验作用域；消耗原业务事务并发送一次 COMMIT。
    pub(crate) async fn commit_changed(self) -> Result<(), DbError> {
        self.transaction.commit_changed().await
    }
}
