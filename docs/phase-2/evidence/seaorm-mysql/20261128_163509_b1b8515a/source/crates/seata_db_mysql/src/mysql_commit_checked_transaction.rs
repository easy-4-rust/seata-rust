use crate::{DbError, MySqlNoChangeCommitError, MySqlPreparedTransaction};
#[cfg(feature = "experimental_changed_commit")]
use crate::{MySqlChangedCommitError, MySqlDataSourceManager, MySqlFirstPhaseReportError};
use seata_at::MySqlJdbcProfile;
#[cfg(feature = "experimental_changed_commit")]
use seata_protocol::{transaction::BranchReportRequest, BranchStatus};
use seata_rm::RegisteredBranch;
#[cfg(feature = "experimental_changed_commit")]
use seata_rm::ResourceClient;

/// 在原业务事务中持有提交前恢复行锁的作用域；有变化分支仍只允许整体回滚或丢弃。
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
    witness_staged: bool,
}
impl MySqlCommitCheckedTransaction {
    /// 参数为在原事务内完成仲裁的准备作用域；仅内部成功路径可构造。
    pub(crate) fn new(prepared: MySqlPreparedTransaction, witness_staged: bool) -> Self {
        Self {
            prepared,
            witness_staged,
        }
    }
    /// 无参数；仅当 004 见证位已在原业务事务内暂存时返回成功。
    /// 旧恢复表和无变化作用域仍可回滚，但不能作为有见证的业务提交依据。
    pub fn require_first_phase_commit_witness(&self) -> Result<(), DbError> {
        if self.witness_staged {
            Ok(())
        } else {
            Err(DbError::Unsupported {
                feature: "explicit first-phase witness migration required",
            })
        }
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
    /// 消耗经仲裁且无变化、无 TC 分支的作用域，发送一次真实本地 COMMIT。
    /// 参数无；成功返回空结果，不向 TC 报告。驱动调用失败均保守归为未知，不重放业务 SQL。
    pub async fn commit_no_change(self) -> Result<(), MySqlNoChangeCommitError> {
        let guard = self
            .prepared
            .phase_one_metrics_sampler()
            .begin(crate::MySqlPhaseOneOperation::CommitNoChange);
        let result = self.commit_no_change_inner().await;
        guard.finish_no_change(&result);
        result
    }

    async fn commit_no_change_inner(self) -> Result<(), MySqlNoChangeCommitError> {
        if self.prepared.registered_branch().is_some()
            || self.prepared.undo_log_count() != 0
            || self.witness_staged
        {
            return Err(MySqlNoChangeCommitError::NotEligible {
                receipt: self.prepared.registered_branch().cloned().map(Box::new),
            });
        }
        self.prepared
            .commit_no_change()
            .await
            .map_err(|source| MySqlNoChangeCommitError::Unknown { source })
    }

    #[cfg(feature = "experimental_changed_commit")]
    /// 实验性消费原仲裁事务；原连接一次 COMMIT 后按同回执独立观察持久见证。
    /// 参数为原配置的资源观察者；返回真实回执仅表示本地提交及见证已确认，不发送 TC 报告。
    /// 发送前拒绝与可能已生效的未知结果分开返回；该特性默认关闭，不构成生产锁安全认证。
    pub async fn commit_changed(
        self,
        observer: &MySqlDataSourceManager,
    ) -> Result<RegisteredBranch, MySqlChangedCommitError> {
        let guard = self
            .prepared
            .phase_one_metrics_sampler()
            .begin(crate::MySqlPhaseOneOperation::CommitChanged);
        let result = self.commit_changed_inner(observer).await;
        guard.finish_changed(&result);
        result
    }

    #[cfg(feature = "experimental_changed_commit")]
    async fn commit_changed_inner(
        self,
        observer: &MySqlDataSourceManager,
    ) -> Result<RegisteredBranch, MySqlChangedCommitError> {
        let receipt = self.prepared.registered_branch().cloned().ok_or(
            MySqlChangedCommitError::NotEligible {
                receipt: None,
                reason: "registered branch receipt required",
            },
        )?;
        if self.prepared.undo_log_count() == 0 {
            return Err(MySqlChangedCommitError::NotEligible {
                receipt: Some(Box::new(receipt)),
                reason: "changed Undo required",
            });
        }
        if !self.witness_staged {
            return Err(MySqlChangedCommitError::NotEligible {
                receipt: Some(Box::new(receipt)),
                reason: "staged first-phase witness required",
            });
        }
        if !observer.accepts_commit_receipt(&receipt, self.prepared.profile()) {
            return Err(MySqlChangedCommitError::NotEligible {
                receipt: Some(Box::new(receipt)),
                reason: "original resource observer required",
            });
        }
        // 独立记录原成功回执与提交意图；无法持久确认时不允许原业务 COMMIT 出站。
        if let Err(source) = crate::mysql_commit_attempt_store::MySqlCommitAttemptStore::arm(
            self.prepared.source(),
            &receipt,
            self.prepared.profile(),
        )
        .await
        {
            let rollback = self.prepared.rollback().await.map_err(Box::new);
            return Err(MySqlChangedCommitError::Arming {
                receipt: Box::new(receipt),
                source,
                rollback,
            });
        }
        // 从这里起驱动可能发送 COMMIT；失败、超时、取消后均不可宣称明确回滚。
        if let Err(source) = self.prepared.commit_changed().await {
            return Err(MySqlChangedCommitError::Unknown {
                receipt: Box::new(receipt),
                source,
            });
        }
        match observer.first_phase_commit_observed(&receipt).await {
            Ok(true) => Ok(receipt),
            Ok(false) => Err(MySqlChangedCommitError::Unknown {
                receipt: Box::new(receipt),
                source: DbError::RecoveryConflict,
            }),
            Err(source) => Err(MySqlChangedCommitError::Unknown {
                receipt: Box::new(receipt),
                source,
            }),
        }
    }
    #[cfg(feature = "experimental_changed_commit")]
    /// 消耗实验仲裁作用域；仅在本地 COMMIT 和独立见证确认后单次报告原 `PhaseOne_Done`。
    /// 参数为同资源观察者及原 RM 客户端；成功返回原回执，失败区分本地结果与报告结果。
    /// 报告响应丢失不重报，不改写本地提交事实；该入口仍不构成生产锁安全认证。
    pub async fn commit_changed_and_report(
        self,
        observer: &MySqlDataSourceManager,
        client: &ResourceClient,
    ) -> Result<RegisteredBranch, MySqlFirstPhaseReportError> {
        let guard = self
            .prepared
            .phase_one_metrics_sampler()
            .begin(crate::MySqlPhaseOneOperation::CommitAndReport);
        let result = self.commit_changed_and_report_inner(observer, client).await;
        guard.finish_report(&result);
        result
    }

    #[cfg(feature = "experimental_changed_commit")]
    async fn commit_changed_and_report_inner(
        self,
        observer: &MySqlDataSourceManager,
        client: &ResourceClient,
    ) -> Result<RegisteredBranch, MySqlFirstPhaseReportError> {
        let receipt = self.commit_changed(observer).await?;
        let identity = receipt.identity();
        let attempt = observer
            .claim_success_report(identity.xid(), identity.branch_id())
            .await
            .map_err(|source| MySqlFirstPhaseReportError::Claim {
                receipt: Box::new(receipt.clone()),
                source,
            })?
            .ok_or_else(|| MySqlFirstPhaseReportError::Claim {
                receipt: Box::new(receipt.clone()),
                source: DbError::RecoveryConflict,
            })?;
        if attempt.identity() != identity || attempt.lock_key() != receipt.lock_key() {
            return Err(MySqlFirstPhaseReportError::Claim {
                receipt: Box::new(receipt),
                source: DbError::RecoveryConflict,
            });
        }
        // 原回执中的可选字段原样构造；报告只发生于独立持久见证确认之后。
        let request = BranchReportRequest {
            xid: Some(identity.xid().to_owned()),
            branch_id: identity.branch_id(),
            status: BranchStatus::PhaseOneDone,
            resource_id: Some(identity.resource().resource_id().to_owned()),
            application_data: identity.application_data().map(str::to_owned),
            branch_type: identity.resource().branch_type(),
        };
        client.branch_report(request).await.map_err(|source| {
            MySqlFirstPhaseReportError::Report {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            }
        })?;
        Ok(receipt)
    }
    /// 消耗整个仲裁作用域，回滚原业务与 Undo 并释放恢复行锁；独立归属记录保留。
    pub async fn rollback(self) -> Result<(), DbError> {
        self.prepared.rollback().await
    }
    #[cfg(feature = "experimental_changed_commit")]
    /// 消耗已仲裁原事务；本地回滚成功后单次报告原 PhaseOne_Failed。
    /// 参数为原 RM 客户端；无分支返回 None，报告失败保留回滚事实及回执。
    pub async fn rollback_and_report_failure(
        self,
        client: &ResourceClient,
    ) -> Result<Option<RegisteredBranch>, crate::MySqlPhaseOneFailureReportError> {
        self.prepared.rollback_and_report_failure(client).await
    }
}
