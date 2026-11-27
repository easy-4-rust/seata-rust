use crate::mysql_tcc_context_reporter::{ContextReportState, MySqlTccContextReporter};
use crate::mysql_tcc_context_store::MySqlTccContextStore;
use crate::{
    mysql_tcc_fence_store::MySqlTccFenceStore, mysql_tcc_transaction::MySqlTccTransaction, DbError,
    MySqlDataSource, MySqlTccTryError,
};
use seata_protocol::{transaction::BranchRegisterRequest, BranchType};
use seata_rm::{
    BranchCallbackObserver, BranchCallbackReceipt, BranchIdentity, BranchPhase, BranchResolution,
    BranchResource, RegisteredBranch, ResourceClient, RmError,
};
use seata_tcc::{
    BusinessActionContext, CommonFenceStatus, FenceDecision, FenceHandler, FenceOperation,
    TccBusinessAction, TccTransaction,
};
use seata_transport::ResourceKey;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::time::{timeout_at, Instant};
/// 同库持久 Fence 的 TCC 三阶段资源，不把回调日志或缺行当作业务完成。
/// 对应 Java: org.apache.seata.rm.tcc.TCCResourceManager 与 org.apache.seata.rm.fence.SpringFenceHandler。
pub struct MySqlTccResourceManager {
    source: MySqlDataSource,
    key: ResourceKey,
    action: Arc<dyn TccBusinessAction>,
    context_reporting: bool,
}
impl fmt::Debug for MySqlTccResourceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlTccResourceManager")
            .field("key", &self.key)
            .field("context_reporting", &self.context_reporting)
            .finish_non_exhaustive()
    }
}
impl MySqlTccResourceManager {
    /// 参数为原数据源、精确动作名及三阶段业务对象；返回不自动建表、登记或清理 Fence 的资源。
    pub fn new(
        source: MySqlDataSource,
        action_name: String,
        action: Arc<dyn TccBusinessAction>,
    ) -> Result<Self, DbError> {
        if action_name.len() > 512 {
            return Err(DbError::Input {
                reason: "TCC action name exceeds bound",
            });
        }
        let key = ResourceKey::new(BranchType::Tcc, action_name).map_err(|_| DbError::Input {
            reason: "invalid TCC action name",
        })?;
        Ok(Self {
            source,
            key,
            action,
            context_reporting: false,
        })
    }
    /// 无参数；显式开启持久上下文报告，要求预先部署 020 迁移；不自动迁移旧 Fence。
    pub fn with_context_reporting(mut self) -> Self {
        self.context_reporting = true;
        self
    }
    const fn protocol_version(&self) -> u16 {
        if self.context_reporting {
            2
        } else {
            1
        }
    }
    fn report_error(
        receipt: RegisteredBranch,
        source: DbError,
        commit_started: bool,
        state: &ContextReportState,
    ) -> MySqlTccTryError {
        let report = state.lock().ok().and_then(|mut value| value.1.take());
        if let Some(source) = report {
            MySqlTccTryError::ContextReport {
                receipt: Box::new(receipt),
                source: Box::new(source),
                context_report_started: Self::report_started(state),
            }
        } else {
            MySqlTccTryError::transaction(receipt, source, commit_started)
        }
    }
    fn report_started(state: &ContextReportState) -> bool {
        state.lock().map(|value| value.0).unwrap_or(true)
    }
    fn context(
        &self,
        xid: &str,
        branch: i64,
        application_data: Option<&str>,
    ) -> Result<BusinessActionContext, DbError> {
        if xid.is_empty()
            || xid.len() > 128
            || xid.trim() != xid
            || xid.chars().any(char::is_control)
        {
            return Err(DbError::Input {
                reason: "TCC XID must be original bounded UTF-8",
            });
        }
        let context = BusinessActionContext::new(
            Some(xid.into()),
            branch,
            Some(self.key.resource_id().into()),
            application_data,
        )
        .map_err(|_| DbError::Input {
            reason: "TCC application context rejected",
        })?;
        if !context.use_common_fence() {
            return Err(DbError::Input {
                reason: "MySQL TCC requires explicit Fence true",
            });
        }
        Ok(context)
    }
    fn identity_context(&self, id: &BranchIdentity) -> Result<BusinessActionContext, DbError> {
        if id.resource() != &self.key {
            return Err(DbError::RecoveryConflict);
        }
        self.context(id.xid(), id.branch_id(), id.application_data())
    }
    /// 参数为正式客户端、原 XID 与原应用信封；单次登记后同事务执行 Try，任何错误保留已知原回执。
    /// 不自动重发登记、Try 或 AT PhaseOne_Done；显式启用后按版本各发送一次动态报告。
    pub async fn register_and_try(
        &self,
        client: &ResourceClient,
        xid: &str,
        application_data: Option<String>,
    ) -> Result<RegisteredBranch, MySqlTccTryError> {
        self.context(xid, 0, application_data.as_deref())
            .map_err(|source| MySqlTccTryError::Local { source })?;
        if self.context_reporting && self.source.control_pool().options().get_max_connections() < 2
        {
            return Err(MySqlTccTryError::Local {
                source: DbError::Invalid {
                    reason: "TCC context reporting requires two control connections",
                },
            });
        }
        let deadline = Instant::now() + self.source.operation_timeout();
        let mut known = None;
        let mut started = false;
        let mut commit_started = false;
        let report_state: ContextReportState = Arc::new(Mutex::new((false, None)));
        let result = timeout_at(deadline, async {
            started = true;
            let receipt = client
                .branch_register_with_receipt(BranchRegisterRequest {
                    xid: Some(xid.into()),
                    branch_type: BranchType::Tcc,
                    resource_id: Some(self.key.resource_id().into()),
                    lock_key: None,
                    application_data,
                })
                .await?;
            known = Some(Box::new(receipt.clone()));
            self.try_inner(
                &receipt,
                &mut commit_started,
                Some(client),
                report_state.clone(),
            )
            .await
            .map_err(|source| {
                Self::report_error(receipt.clone(), source, commit_started, &report_state)
            })?;
            Ok(receipt)
        })
        .await;
        if Instant::now() >= deadline {
            return Err(MySqlTccTryError::Deadline {
                receipt: known,
                registration_may_have_effect: started,
                try_commit_may_have_effect: commit_started,
                context_report_may_have_effect: Self::report_started(&report_state),
            });
        }
        result.map_err(|_| MySqlTccTryError::Deadline {
            receipt: known,
            registration_may_have_effect: started,
            try_commit_may_have_effect: commit_started,
            context_report_may_have_effect: Self::report_started(&report_state),
        })?
    }
    /// 参数为正式客户端签发的原成功回执；任何已有 Fence 都拒绝重复 Try，不重造身份。
    pub async fn try_registered(
        &self,
        receipt: RegisteredBranch,
    ) -> Result<RegisteredBranch, MySqlTccTryError> {
        self.try_registered_inner(None, receipt).await
    }
    /// 参数为正式客户端与原成功回执；执行显式动态报告 Try，不重发登记或构造新回执。
    pub async fn try_registered_with_context(
        &self,
        client: &ResourceClient,
        receipt: RegisteredBranch,
    ) -> Result<RegisteredBranch, MySqlTccTryError> {
        if !self.context_reporting {
            return Err(MySqlTccTryError::transaction(
                receipt,
                DbError::Invalid {
                    reason: "TCC context reporting was not explicitly enabled",
                },
                false,
            ));
        }
        self.try_registered_inner(Some(client), receipt).await
    }
    async fn try_registered_inner(
        &self,
        client: Option<&ResourceClient>,
        receipt: RegisteredBranch,
    ) -> Result<RegisteredBranch, MySqlTccTryError> {
        let deadline = Instant::now() + self.source.operation_timeout();
        let mut commit_started = false;
        let report_state: ContextReportState = Arc::new(Mutex::new((false, None)));
        let result = timeout_at(
            deadline,
            self.try_inner(&receipt, &mut commit_started, client, report_state.clone()),
        )
        .await;
        if Instant::now() >= deadline {
            return Err(MySqlTccTryError::Deadline {
                receipt: Some(Box::new(receipt)),
                registration_may_have_effect: false,
                try_commit_may_have_effect: commit_started,
                context_report_may_have_effect: Self::report_started(&report_state),
            });
        }
        result
            .map_err(|_| MySqlTccTryError::Deadline {
                receipt: Some(Box::new(receipt.clone())),
                registration_may_have_effect: false,
                try_commit_may_have_effect: commit_started,
                context_report_may_have_effect: Self::report_started(&report_state),
            })?
            .map_err(|source| {
                Self::report_error(receipt.clone(), source, commit_started, &report_state)
            })?;
        Ok(receipt)
    }
    async fn try_inner(
        &self,
        receipt: &RegisteredBranch,
        commit_started: &mut bool,
        client: Option<&ResourceClient>,
        report_state: ContextReportState,
    ) -> Result<(), DbError> {
        let id = receipt.identity();
        let mut context = self.identity_context(id)?;
        if receipt.lock_key().is_some() {
            return Err(DbError::RecoveryConflict);
        }
        if self.context_reporting && self.source.control_pool().options().get_max_connections() < 2
        {
            return Err(DbError::Invalid {
                reason: "TCC context reporting requires two control connections",
            });
        }
        let reporter = if self.context_reporting {
            let client = client.ok_or(DbError::Input {
                reason: "TCC context reporting requires explicit client",
            })?;
            MySqlTccContextStore::create(&self.source, receipt).await?;
            context.set_delay_report(Some(true));
            context.enable_action_context_tracking();
            let initial = context.to_application_data().map_err(|_| DbError::Input {
                reason: "TCC context rejected",
            })?;
            Some(MySqlTccContextReporter::new(
                self.source.clone(),
                client.clone(),
                receipt.clone(),
                initial,
                report_state,
            ))
        } else {
            None
        };
        let mut tx = MySqlTccTransaction::begin(&self.source).await?;
        if let Some(reporter) = reporter {
            tx.attach_reporter(reporter);
        }
        let state =
            MySqlTccFenceStore::read(tx.control()?, &self.source, id, self.protocol_version())
                .await?;
        if FenceHandler::plan(FenceOperation::Try, state)
            != FenceDecision::InsertAndInvoke(CommonFenceStatus::Tried)
        {
            return Err(DbError::RecoveryConflict);
        }
        if !self.context_reporting {
            MySqlTccContextStore::assert_no_dynamic_origin(tx.control()?, &self.source, id, true)
                .await?;
        }
        MySqlTccFenceStore::insert(
            tx.control()?,
            &self.source,
            id,
            CommonFenceStatus::Tried,
            self.protocol_version(),
        )
        .await?;
        let accepted = if self.context_reporting {
            self.action
                .try_action_with_context(&mut context, &mut tx)
                .await
        } else {
            self.action.try_action(&context, &mut tx).await
        };
        if self.context_reporting {
            tx.report_context(&mut context)
                .await
                .map_err(|_| DbError::Input {
                    reason: "TCC final context report failed",
                })?;
        }
        if !accepted.map_err(|_| DbError::Input {
            reason: "TCC Try action failed",
        })? {
            return Err(DbError::Input {
                reason: "TCC Try action refused",
            });
        }
        tx.commit_tracked(commit_started).await
    }
    async fn persisted_identity(
        &self,
        tx: &mut MySqlTccTransaction,
        id: &BranchIdentity,
    ) -> Result<BranchIdentity, DbError> {
        if !self.context_reporting {
            if MySqlTccFenceStore::read(tx.control()?, &self.source, id, 1)
                .await?
                .is_none()
            {
                MySqlTccContextStore::assert_no_dynamic_origin(
                    tx.control()?,
                    &self.source,
                    id,
                    true,
                )
                .await?;
            }
            return Ok(id.clone());
        }
        let status: Option<u8> = sqlx::query_scalar(
            "SELECT status FROM seata_rs_tcc_fence_v1 WHERE xid=? AND branch_id=? FOR UPDATE",
        )
        .bind(id.xid().as_bytes())
        .bind(id.branch_id())
        .fetch_optional(&mut **tx.control()?)
        .await
        .map_err(|e| DbError::driver("TCC context Fence lock", e))?;
        let last = MySqlTccContextStore::latest(tx.control()?, &self.source, id).await?;
        let original = if let Some(last) = last {
            let app = id.application_data().ok_or(DbError::RecoveryConflict)?;
            if !MySqlTccContextStore::known(tx.control()?, &self.source, &last, app).await? {
                return Err(DbError::RecoveryConflict);
            }
            if matches!(status, Some(1..=3)) && app != last.application_data {
                return Err(DbError::Input {
                    reason: "stale TCC report context",
                });
            }
            last.original
        } else {
            if matches!(status, Some(1..=3)) {
                return Err(DbError::RecoveryConflict);
            }
            id.clone()
        };
        MySqlTccFenceStore::read(
            tx.control()?,
            &self.source,
            &original,
            self.protocol_version(),
        )
        .await?;
        Ok(original)
    }
    async fn finish(
        &self,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchResolution, DbError> {
        let context = self.identity_context(id)?;
        let mut tx = MySqlTccTransaction::begin(&self.source).await?;
        let original = self.persisted_identity(&mut tx, id).await?;
        let state = MySqlTccFenceStore::read(
            tx.control()?,
            &self.source,
            &original,
            self.protocol_version(),
        )
        .await?;
        let operation = match phase {
            BranchPhase::Commit => FenceOperation::Confirm,
            BranchPhase::Rollback => FenceOperation::Cancel,
        };
        match FenceHandler::plan(operation, state) {
            FenceDecision::AlreadyApplied => {
                tx.commit().await?;
                Ok(BranchResolution::Applied)
            }
            FenceDecision::UpdateAndInvoke { expected, next } => {
                // 持原行锁排除旧尝试；状态与业务在一个事务中生效，未知提交只能再核对原 Fence。
                MySqlTccFenceStore::update(tx.control()?, &original, expected, next).await?;
                let result = match phase {
                    BranchPhase::Commit => self.action.confirm(&context, &mut tx).await,
                    BranchPhase::Rollback => self.action.cancel(&context, &mut tx).await,
                };
                if !result.map_err(|_| DbError::Input {
                    reason: "TCC phase-two action failed",
                })? {
                    return Err(DbError::Input {
                        reason: "TCC phase-two action refused",
                    });
                }
                tx.commit().await?;
                Ok(BranchResolution::Applied)
            }
            // 缺行 Cancel 只能由原连接 observer 唯一插入屏障；手工 apply/recover 不授予该权限。
            FenceDecision::InsertSuspended | FenceDecision::MissingTry | FenceDecision::Refuse => {
                Ok(BranchResolution::Unknown)
            }
            FenceDecision::InsertAndInvoke(_) => Err(DbError::RecoveryConflict),
        }
    }
    async fn resolve(
        &self,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchResolution, RmError> {
        let deadline = Instant::now() + self.source.operation_timeout();
        let result = timeout_at(deadline, self.finish(id, phase)).await;
        if Instant::now() >= deadline {
            return Ok(BranchResolution::Unknown);
        }
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(DbError::RecoveryConflict)) => Err(RmError::IdentityConflict),
            _ => Ok(BranchResolution::Unknown),
        }
    }
    async fn observe_rollback(&self, receipt: &BranchCallbackReceipt) -> Result<(), DbError> {
        self.identity_context(receipt.identity())?;
        let mut tx = MySqlTccTransaction::begin(&self.source).await?;
        let original = self.persisted_identity(&mut tx, receipt.identity()).await?;
        if MySqlTccFenceStore::read(
            tx.control()?,
            &self.source,
            &original,
            self.protocol_version(),
        )
        .await?
        .is_none()
        {
            MySqlTccFenceStore::insert(
                tx.control()?,
                &self.source,
                &original,
                CommonFenceStatus::Suspended,
                self.protocol_version(),
            )
            .await?;
        }
        tx.commit().await
    }
}
impl BranchResource for MySqlTccResourceManager {
    fn verify_callback_completion<'a>(
        &'a self,
        receipt: &'a BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), RmError>> + Send + 'a>> {
        Box::pin(async move {
            self.identity_context(receipt.identity())
                .map_err(|_| RmError::IdentityConflict)?;
            let deadline = Instant::now() + self.source.operation_timeout();
            let result = timeout_at(deadline, async {
                let mut tx = MySqlTccTransaction::begin(&self.source).await?;
                let original = self.persisted_identity(&mut tx, receipt.identity()).await?;
                let state = MySqlTccFenceStore::read(
                    tx.control()?,
                    &self.source,
                    &original,
                    self.protocol_version(),
                )
                .await?;
                let terminal = matches!(
                    (receipt.phase(), state),
                    (BranchPhase::Commit, Some(CommonFenceStatus::Committed))
                        | (
                            BranchPhase::Rollback,
                            Some(CommonFenceStatus::Rollbacked | CommonFenceStatus::Suspended)
                        )
                );
                if !terminal {
                    return Err(DbError::Input {
                        reason: "TCC completed journal lacks terminal Fence",
                    });
                }
                tx.rollback_read_scope().await
            })
            .await;
            if Instant::now() >= deadline {
                return Err(RmError::Unavailable {
                    reason: "TCC completion verification deadline",
                });
            }
            match result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(DbError::RecoveryConflict)) => Err(RmError::IdentityConflict),
                _ => Err(RmError::Unavailable {
                    reason: "TCC completion verification unavailable",
                }),
            }
        })
    }
    fn callback_identity<'a>(
        &'a self,
        receipt: &'a BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<BranchIdentity, RmError>> + Send + 'a>> {
        Box::pin(async move {
            self.identity_context(receipt.identity())
                .map_err(|_| RmError::IdentityConflict)?;
            let result = async {
                let mut tx = MySqlTccTransaction::begin(&self.source).await?;
                let original = if self.context_reporting {
                    self.persisted_identity(&mut tx, receipt.identity()).await?
                } else {
                    // 静态预检不等待旧 Try 的行锁；先拒绝动态来源，避免把新参数写成原日志身份。
                    if MySqlTccFenceStore::read_snapshot(
                        tx.control()?,
                        &self.source,
                        receipt.identity(),
                        1,
                    )
                    .await?
                    .is_none()
                    {
                        MySqlTccContextStore::assert_no_dynamic_origin(
                            tx.control()?,
                            &self.source,
                            receipt.identity(),
                            false,
                        )
                        .await.ok();
                    }
                    receipt.identity().clone()
                };
                tx.rollback_read_scope().await?;
                Ok::<_, DbError>(original)
            }
            .await;
            result.map_err(|error| {
                if matches!(error, DbError::RecoveryConflict) {
                    RmError::IdentityConflict
                } else {
                    RmError::Unavailable {
                        reason: "TCC callback context unavailable",
                    }
                }
            })
        })
    }
    fn callback_observer(&self) -> Option<&dyn BranchCallbackObserver> {
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
        Box::pin(self.resolve(identity, phase))
    }
    fn recover<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(self.resolve(identity, phase))
    }
}
impl BranchCallbackObserver for MySqlTccResourceManager {
    fn observe<'a>(
        &'a self,
        receipt: &'a BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), RmError>> + Send + 'a>> {
        Box::pin(async move {
            if receipt.phase() != BranchPhase::Rollback {
                return Ok(());
            }
            let deadline = Instant::now() + self.source.operation_timeout();
            let result = timeout_at(deadline, self.observe_rollback(receipt)).await;
            if Instant::now() >= deadline {
                return Err(RmError::Unavailable {
                    reason: "TCC callback Fence deadline",
                });
            }
            result
                .map_err(|_| RmError::Unavailable {
                    reason: "TCC callback Fence deadline",
                })?
                .map_err(|e| match e {
                    DbError::RecoveryConflict => RmError::IdentityConflict,
                    _ => RmError::Unavailable {
                        reason: "TCC callback Fence unresolved",
                    },
                })
        })
    }
}
