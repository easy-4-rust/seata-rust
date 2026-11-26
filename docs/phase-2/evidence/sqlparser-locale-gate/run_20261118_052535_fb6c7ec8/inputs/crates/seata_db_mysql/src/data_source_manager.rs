use crate::{
    recovery_action::RecoveryAction, rm_branch_store::RmBranchStore, DbError, MySqlDataSource,
    MySqlRecoveryError,
};
use seata_at::MySqlJdbcProfile;
use seata_protocol::BranchType;
use seata_rm::{
    BranchIdentity, BranchPhase, BranchResolution, BranchResource, RegisteredBranch, RmError,
};
use seata_transport::ResourceKey;
use std::{future::Future, pin::Pin};
use tokio::time::{timeout_at, Instant};

/// 原资源精确匹配的 MySQL RM 回滚适配器；独立恢复行与补偿原子提交。
/// 对应 Java: org.apache.seata.rm.datasource.DataSourceManager#branchRollback（部分实现）。
/// 提交清理要求显式迁移及正式回调的持久授权；不提供一阶段业务 COMMIT，也不返回 NotApplied。
#[derive(Clone, Debug)]
pub struct MySqlDataSourceManager {
    source: MySqlDataSource,
    key: ResourceKey,
    profile: MySqlJdbcProfile,
}
impl MySqlDataSourceManager {
    /// 消耗参数原事务，按实际镜像单次登记并同事务准备 Undo；返回只读身份／回滚作用域。
    /// 原 XID 和应用数据不规范化；失败或取消不允许取回原事务继续写入，不提供业务 COMMIT。
    pub async fn prepare_transaction(
        &self,
        transaction: crate::MySqlLocalTransaction,
        client: &seata_rm::ResourceClient,
        xid: &str,
        application_data: Option<String>,
    ) -> Result<crate::MySqlPreparedTransaction, crate::MySqlPrepareError> {
        crate::mysql_transaction_preparer::MySqlTransactionPreparer::prepare(
            self,
            transaction,
            client,
            xid,
            application_data,
            self.profile,
            self.source.operation_timeout(),
        )
        .await
    }
    /// 参数为已连接单主机数据源与固定 JDBC 配置；返回精确资源适配器，不执行迁移或注册。
    pub fn new(source: MySqlDataSource, profile: MySqlJdbcProfile) -> Result<Self, DbError> {
        let key = ResourceKey::new(BranchType::At, source.resource_id()?).map_err(|_| {
            DbError::Invalid {
                reason: "MySQL RM resource identity",
            }
        })?;
        Ok(Self {
            source,
            key,
            profile,
        })
    }
    fn valid(&self, id: &BranchIdentity) -> bool {
        id.resource() == &self.key && id.xid().len() <= 128
    }
    /// 参数为正式客户端成功注册回执；独立持久登记原身份与锁键，重复核对，不证明业务提交。
    /// 取消或 COMMIT 确认丢失后可持同一回执核对登记，禁止重新向 TC 申请编号。
    pub async fn enroll(&self, receipt: &RegisteredBranch) -> Result<(), MySqlRecoveryError> {
        let id = receipt.identity();
        if !self.valid(id) {
            return Err(MySqlRecoveryError::InvalidIdentity);
        }
        self.control(
            id,
            RecoveryAction::Enroll(receipt),
            Instant::now() + self.source.operation_timeout(),
        )
        .await
        .map(|_| ())
    }
    async fn control(
        &self,
        id: &BranchIdentity,
        action: RecoveryAction<'_>,
        deadline: Instant,
    ) -> Result<BranchResolution, MySqlRecoveryError> {
        let mut commit_attempted = false;
        let value = timeout_at(deadline, async {
            let mut connection = self
                .source
                .control_pool()
                .acquire()
                .await
                .map_err(|e| DbError::driver("RM recovery acquire", e))?;
            // 任何 await 取消都关闭实际租约；不将半事务/半协议连接放回池。
            connection.close_on_drop();
            let mut tx = sqlx::Transaction::begin(connection, None)
                .await
                .map_err(|e| DbError::driver("RM recovery begin", e))?;
            let schema = RmBranchStore::validate_table(&mut tx, self.source.database()).await?;
            let (resolution, commit) = match action {
                RecoveryAction::Enroll(receipt) => {
                    RmBranchStore::enroll(&mut tx, receipt, self.profile, schema).await?;
                    (BranchResolution::Unknown, true)
                }
                RecoveryAction::Observe(receipt) => {
                    let commit =
                        RmBranchStore::observe_callback(&mut tx, receipt, self.profile, schema)
                            .await?;
                    (BranchResolution::Unknown, commit)
                }
                other => match RmBranchStore::lock(&mut tx, id, self.profile, schema).await? {
                    None => (BranchResolution::Unknown, false),
                    Some(record) => match other {
                        RecoveryAction::Bind(phase) => {
                            if record.phase != 0 && record.phase != RmBranchStore::phase(phase) {
                                return Err(DbError::RecoveryConflict);
                            }
                            if phase == BranchPhase::Commit
                                && schema == crate::recovery_schema::RecoverySchema::CommitDecision
                                && !record.commit_authorized
                            {
                                // 旧 phase1 也可能来自公开手工调用，不能据此合成正式 TC 决定。
                                (BranchResolution::Unknown, false)
                            } else if record.phase == 0 {
                                RmBranchStore::bind(&mut tx, id, phase).await?;
                                (BranchResolution::Unknown, true)
                            } else {
                                (
                                    if record.terminal {
                                        BranchResolution::Applied
                                    } else {
                                        BranchResolution::Unknown
                                    },
                                    false,
                                )
                            }
                        }
                        RecoveryAction::Rollback => {
                            if record.phase != 2 {
                                return Err(DbError::RecoveryConflict);
                            }
                            if record.terminal {
                                (BranchResolution::Applied, false)
                            } else {
                                // 锁内无终态证明本适配器没有提交旧补偿；原 Undo 检查、补偿与终态共用这一物理事务。
                                crate::undo::AbstractUndoLogManager::prepare(
                                    &mut tx,
                                    self.source.database(),
                                    id.xid(),
                                    id.branch_id(),
                                    self.profile,
                                )
                                .await?;
                                RmBranchStore::complete_rollback(&mut tx, id).await?;
                                (BranchResolution::Applied, true)
                            }
                        }
                        RecoveryAction::Commit => {
                            if record.phase == 2 {
                                return Err(DbError::RecoveryConflict);
                            }
                            if schema != crate::recovery_schema::RecoverySchema::CommitDecision
                                || !record.commit_authorized
                            {
                                (BranchResolution::Unknown, false)
                            } else if record.terminal {
                                (BranchResolution::Applied, false)
                            } else {
                                crate::mysql::MySqlUndoLogManager::delete_committed(
                                    &mut tx,
                                    self.source.database(),
                                    id.xid(),
                                    id.branch_id(),
                                )
                                .await?;
                                RmBranchStore::complete_commit(&mut tx, id).await?;
                                (BranchResolution::Applied, true)
                            }
                        }
                        RecoveryAction::Enroll(_) | RecoveryAction::Observe(_) => {
                            return Err(DbError::Input {
                                reason: "invalid recovery action",
                            })
                        }
                    },
                },
            };
            if Instant::now() >= deadline {
                return Err(DbError::Deadline {
                    operation: "RM recovery",
                });
            }
            if commit {
                commit_attempted = true;
                tx.commit()
                    .await
                    .map_err(|e| DbError::driver("RM recovery commit", e))?;
            } else {
                tx.rollback()
                    .await
                    .map_err(|e| DbError::driver("RM recovery read rollback", e))?;
            }
            Ok(resolution)
        })
        .await;
        let value = if Instant::now() >= deadline {
            Err(DbError::Deadline {
                operation: "RM recovery",
            })
        } else {
            value.unwrap_or(Err(DbError::Deadline {
                operation: "RM recovery",
            }))
        };
        value.map_err(|source| {
            if commit_attempted {
                MySqlRecoveryError::CommitOutcomeUnknown {
                    identity: Box::new(id.clone()),
                    source,
                }
            } else {
                MySqlRecoveryError::BeforeCommit {
                    identity: Box::new(id.clone()),
                    source,
                }
            }
        })
    }
    async fn reconcile(
        &self,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchResolution, RmError> {
        if !self.valid(id) {
            return Err(RmError::Invalid {
                reason: "MySQL RM identity does not match resource or XID capacity",
            });
        }
        // 两个独立控制事务共用一个绝对期限。方向一旦确认，即使补偿失败也不可切换。
        let deadline = Instant::now() + self.source.operation_timeout();
        if let Err(error) = self
            .control(id, RecoveryAction::Bind(phase), deadline)
            .await
        {
            return Self::resolution(error);
        }
        let action = match phase {
            BranchPhase::Commit => RecoveryAction::Commit,
            BranchPhase::Rollback => RecoveryAction::Rollback,
        };
        self.control(id, action, deadline)
            .await
            .or_else(Self::resolution)
    }
    fn resolution(error: MySqlRecoveryError) -> Result<BranchResolution, RmError> {
        match error {
            MySqlRecoveryError::BeforeCommit {
                source: DbError::RecoveryConflict,
                ..
            } => Err(RmError::IdentityConflict),
            _ => Ok(BranchResolution::Unknown),
        }
    }
}
impl BranchResource for MySqlDataSourceManager {
    fn callback_observer(&self) -> Option<&dyn seata_rm::BranchCallbackObserver> {
        Some(self)
    }
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn apply<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(self.reconcile(identity, phase))
    }
    fn recover<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(self.reconcile(identity, phase))
    }
}
impl seata_rm::BranchCallbackObserver for MySqlDataSourceManager {
    fn observe<'a>(
        &'a self,
        receipt: &'a seata_rm::BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), RmError>> + Send + 'a>> {
        Box::pin(async move {
            if !self.valid(receipt.identity()) {
                return Err(RmError::Invalid {
                    reason: "MySQL callback identity does not match resource or XID capacity",
                });
            }
            self.control(
                receipt.identity(),
                RecoveryAction::Observe(receipt),
                Instant::now() + self.source.operation_timeout(),
            )
            .await
            .map(|_| ())
            .map_err(|error| match error {
                MySqlRecoveryError::BeforeCommit {
                    source: DbError::RecoveryConflict,
                    ..
                } => RmError::IdentityConflict,
                _ => RmError::Unavailable {
                    reason: "MySQL callback ownership not durably confirmed",
                },
            })
        })
    }
}
