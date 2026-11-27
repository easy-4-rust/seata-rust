use crate::{
    mysql_xa_branch_store::MySqlXaBranchStore, DbError, MySqlDataSource, MySqlXaDataSource,
    MySqlXaEnlistError, MySqlXaEnlistedTransaction,
};
use seata_protocol::{transaction::BranchRegisterRequest, BranchType};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchResolution, BranchResource, RegisteredBranch,
    ResourceClient, RmError,
};
use seata_transport::ResourceKey;
use seata_xa::XaBranchXid;
use std::{future::Future, pin::Pin};
use tokio::time::{timeout_at, Instant};
/// 精确 XA 模式的正式 RM 适配器；先持久原登记，再启动一次分支，同方向原生结束后确认终态。
/// 对应 Java: org.apache.seata.rm.datasource.xa.ResourceManagerXA（选定登记/两阶段路径）。
#[derive(Clone, Debug)]
pub struct MySqlXaResourceManager {
    pub(crate) native: MySqlXaDataSource,
    key: ResourceKey,
}
impl MySqlXaResourceManager {
    /// 参数为已核对原物理 schema/TLS 的数据源；返回至少两个池槽且原 JDBC 资源精确匹配的 XA 管理器。
    pub fn new(source: MySqlDataSource) -> Result<Self, DbError> {
        let resource = source.resource_id()?;
        if resource.len() > 512 || source.control_pool().options().get_max_connections() < 2 {
            return Err(DbError::Invalid {
                reason: "managed XA requires bounded JDBC identity and two pool slots",
            });
        }
        let key = ResourceKey::new(BranchType::Xa, resource).map_err(|_| DbError::Invalid {
            reason: "exact XA JDBC resource required",
        })?;
        Ok(Self {
            native: MySqlXaDataSource::new(source),
            key,
        })
    }
    /// 参数为正式 RM、原 XID 与原可选应用数据；单次登记获得真实编号并持久防重放记录后开始 XA。
    /// 不发送 PhaseOne_Done；未知登记不合成编号，任何控制 COMMIT 未确认均不开始原生分支。
    pub async fn register_and_start(
        &self,
        client: &ResourceClient,
        xid: &str,
        application_data: Option<String>,
    ) -> Result<MySqlXaEnlistedTransaction, MySqlXaEnlistError> {
        if xid.is_empty()
            || xid.len() > 64
            || xid.trim() != xid
            || xid.chars().any(char::is_control)
        {
            return Err(MySqlXaEnlistError::Local {
                source: DbError::Input {
                    reason: "managed XA XID must be original bounded UTF-8",
                },
            });
        }
        let deadline = Instant::now() + self.native.source.operation_timeout();
        let mut known = None;
        let mut started = false;
        let result = timeout_at(deadline, async {
            started = true;
            let receipt = client
                .branch_register_with_receipt(BranchRegisterRequest {
                    xid: Some(xid.into()),
                    branch_type: BranchType::Xa,
                    resource_id: Some(self.key.resource_id().into()),
                    application_data,
                    lock_key: None,
                })
                .await?;
            known = Some(Box::new(receipt.clone()));
            self.start_inner(receipt).await
        })
        .await;
        if Instant::now() >= deadline {
            return Err(MySqlXaEnlistError::Deadline {
                receipt: known,
                registration_may_have_effect: started,
            });
        }
        result.map_err(|_| MySqlXaEnlistError::Deadline {
            receipt: known,
            registration_may_have_effect: started,
        })?
    }
    /// 参数为正式客户端签发的原 XA 登记回执；其克隆及已完成分支也只能成功启动一次。
    pub async fn start_registered(
        &self,
        receipt: RegisteredBranch,
    ) -> Result<MySqlXaEnlistedTransaction, MySqlXaEnlistError> {
        let deadline = Instant::now() + self.native.source.operation_timeout();
        let known = receipt.clone();
        let result = timeout_at(deadline, self.start_inner(receipt)).await;
        if Instant::now() >= deadline {
            return Err(MySqlXaEnlistError::Deadline {
                receipt: Some(Box::new(known)),
                registration_may_have_effect: true,
            });
        }
        result.map_err(|_| MySqlXaEnlistError::Deadline {
            receipt: Some(Box::new(known)),
            registration_may_have_effect: true,
        })?
    }
    async fn start_inner(
        &self,
        receipt: RegisteredBranch,
    ) -> Result<MySqlXaEnlistedTransaction, MySqlXaEnlistError> {
        let known = || Box::new(receipt.clone());
        self.validate(receipt.identity())
            .map_err(|source| MySqlXaEnlistError::Control {
                receipt: known(),
                source,
            })?;
        if receipt.lock_key().is_some() {
            return Err(MySqlXaEnlistError::Control {
                receipt: known(),
                source: DbError::RecoveryConflict,
            });
        }
        MySqlXaBranchStore::claim(&self.native.source, &receipt)
            .await
            .map_err(|source| MySqlXaEnlistError::Control {
                receipt: known(),
                source,
            })?;
        let (mut gate, row) = MySqlXaBranchStore::lock(&self.native.source, receipt.identity())
            .await
            .map_err(|source| MySqlXaEnlistError::Control {
                receipt: known(),
                source,
            })?;
        if !row.is_some_and(|r| r.phase == 0 && !r.completed) {
            return Err(MySqlXaEnlistError::Control {
                receipt: known(),
                source: DbError::RecoveryConflict,
            });
        }
        MySqlXaBranchStore::validate_table(
            &mut gate,
            &self.native.source,
            crate::mysql_xa_session_fence::MySqlXaSessionFence::GUARD,
        )
        .await
        .map_err(|source| MySqlXaEnlistError::Control {
            receipt: known(),
            source,
        })?;
        let identity = Self::xa_id(receipt.identity());
        let mut transaction =
            self.native
                .start(identity)
                .await
                .map_err(|source| MySqlXaEnlistError::Native {
                    receipt: known(),
                    source,
                })?;
        transaction
            .insert_session_guard(receipt.identity())
            .await
            .map_err(|source| MySqlXaEnlistError::Native {
                receipt: known(),
                source,
            })?;
        gate.rollback()
            .await
            .map_err(|e| MySqlXaEnlistError::Control {
                receipt: known(),
                source: DbError::driver("XA start gate release", e),
            })?;
        Ok(MySqlXaEnlistedTransaction::new(
            self.clone(),
            receipt,
            transaction,
        ))
    }
    /// 参数为完整原回调身份；返回原模式/资源/字节上限校验结果，不产生登记或数据库动作。
    pub(crate) fn validate(&self, id: &BranchIdentity) -> Result<(), DbError> {
        if id.resource() != &self.key
            || id.xid().len() > 64
            || id.application_data().is_some_and(|v| v.len() > 1_048_576)
        {
            return Err(DbError::RecoveryConflict);
        }
        Self::xa_id(id)
            .mysql_arguments()
            .map_err(|_| DbError::RecoveryConflict)?;
        Ok(())
    }
    /// 参数为已核对原身份；返回相同 Seata XA 两分量的值对象，不重造登记权限。
    pub(crate) fn xa_id(id: &BranchIdentity) -> XaBranchXid {
        XaBranchXid::new(Some(id.xid().into()), id.branch_id())
    }
    async fn finish(
        &self,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchResolution, DbError> {
        self.validate(id)?;
        if !MySqlXaBranchStore::bind(&self.native.source, id, phase).await? {
            return Ok(BranchResolution::Unknown);
        }
        for _ in 0..2 {
            let (mut gate, row) = MySqlXaBranchStore::lock(&self.native.source, id).await?;
            let row = row.ok_or(DbError::RecoveryConflict)?;
            if row.phase != MySqlXaBranchStore::phase(phase) {
                return Err(DbError::RecoveryConflict);
            }
            if row.completed {
                gate.rollback()
                    .await
                    .map_err(|e| DbError::driver("XA completed gate release", e))?;
                return Ok(BranchResolution::Applied);
            }
            let xid = Self::xa_id(id);
            let prepared = self.native.prepared_observed(&xid).await;
            if matches!(prepared, Ok(false))
                && phase == BranchPhase::Commit
                && crate::mysql_xa_commit_witness::MySqlXaCommitWitness::committed(
                    &self.native.source,
                    id,
                )
                .await?
            {
                // 正向见证与业务同 XA 原子提交；只结束原持久决定，不重发原生命令。
                MySqlXaBranchStore::complete(&mut gate, id, phase).await?;
                gate.commit()
                    .await
                    .map_err(|e| DbError::driver("XA witnessed terminal commit", e))?;
                return Ok(BranchResolution::Applied);
            }
            if matches!(prepared, Ok(false))
                && phase == BranchPhase::Rollback
                && crate::mysql_xa_prepare_evidence::MySqlXaPrepareEvidence::rollback_armed(
                    &mut gate,
                    &self.native.source,
                    id,
                )
                .await?
            {
                if crate::mysql_xa_commit_witness::MySqlXaCommitWitness::committed(
                    &self.native.source,
                    id,
                )
                .await?
                {
                    return Err(DbError::RecoveryConflict);
                }
                MySqlXaBranchStore::complete(&mut gate, id, phase).await?;
                gate.commit()
                    .await
                    .map_err(|e| DbError::driver("XA proved rollback terminal commit", e))?;
                return Ok(BranchResolution::Applied);
            }
            if matches!(prepared, Ok(false))
                && phase == BranchPhase::Rollback
                && crate::mysql_xa_session_fence::MySqlXaSessionFence::fence(
                    &mut gate,
                    &self.native.source,
                    id,
                )
                .await?
            {
                if !matches!(self.native.prepared_observed(&xid).await, Ok(false)) {
                    return Ok(BranchResolution::Unknown);
                }
                if crate::mysql_xa_commit_witness::MySqlXaCommitWitness::committed(
                    &self.native.source,
                    id,
                )
                .await?
                {
                    return Err(DbError::RecoveryConflict);
                }
                MySqlXaBranchStore::complete(&mut gate, id, phase).await?;
                gate.commit()
                    .await
                    .map_err(|e| DbError::driver("XA active guard terminal commit", e))?;
                return Ok(BranchResolution::Applied);
            }
            if !matches!(prepared, Ok(true)) {
                gate.rollback()
                    .await
                    .map_err(|e| DbError::driver("XA unknown gate release", e))?;
                return Ok(BranchResolution::Unknown);
            }
            if phase == BranchPhase::Rollback
                && crate::mysql_xa_prepare_evidence::MySqlXaPrepareEvidence::arm_rollback(
                    &mut gate,
                    &self.native.source,
                    id,
                )
                .await?
                    == Some(true)
            {
                // 此时已实际观察 Prepared；先确认结束意图，再重新获取原门闩及原生状态。
                gate.commit()
                    .await
                    .map_err(|e| DbError::driver("XA Prepared rollback intent commit", e))?;
                continue;
            }
            // 只有完整原 Prepared 和持久同方向都已确认才结束；不重新 START 或执行业务。
            let result = match phase {
                BranchPhase::Commit => self.native.commit_prepared(&xid).await,
                BranchPhase::Rollback => self.native.rollback_prepared(&xid).await,
            };
            if result.is_err() {
                return Ok(BranchResolution::Unknown);
            }
            MySqlXaBranchStore::complete(&mut gate, id, phase).await?;
            gate.commit()
                .await
                .map_err(|e| DbError::driver("XA terminal commit", e))?;
            return Ok(BranchResolution::Applied);
        }
        Ok(BranchResolution::Unknown)
    }
    async fn resolve(
        &self,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchResolution, RmError> {
        let deadline = Instant::now() + self.native.source.operation_timeout();
        let result = timeout_at(deadline, self.finish(id, phase)).await;
        if Instant::now() >= deadline {
            return Err(RmError::Unavailable {
                reason: "XA resource deadline; original outcome remains unresolved",
            });
        }
        result
            .map_err(|_| RmError::Unavailable {
                reason: "XA resource deadline; original outcome remains unresolved",
            })?
            .map_err(|error| match error {
                DbError::RecoveryConflict => RmError::IdentityConflict,
                _ => RmError::Unavailable {
                    reason: "XA original identity or control result remains unresolved",
                },
            })
    }
}
impl BranchResource for MySqlXaResourceManager {
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

impl seata_rm::BranchCallbackObserver for MySqlXaResourceManager {
    fn observe<'a>(
        &'a self,
        receipt: &'a seata_rm::BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), RmError>> + Send + 'a>> {
        Box::pin(async move {
            if receipt.phase() != BranchPhase::Rollback {
                return Ok(());
            }
            self.validate(receipt.identity())
                .map_err(|_| RmError::IdentityConflict)?;
            let deadline = Instant::now() + self.native.source.operation_timeout();
            let result = timeout_at(
                deadline,
                crate::mysql_xa_early_rollback::MySqlXaEarlyRollback::observe(
                    &self.native,
                    receipt,
                ),
            )
            .await;
            if Instant::now() >= deadline {
                return Err(RmError::Unavailable {
                    reason: "XA early callback deadline; original outcome unresolved",
                });
            }
            result
                .map_err(|_| RmError::Unavailable {
                    reason: "XA early callback deadline; original outcome unresolved",
                })?
                .map_err(|error| match error {
                    DbError::RecoveryConflict => RmError::IdentityConflict,
                    _ => RmError::Unavailable {
                        reason: "XA early callback control outcome unresolved",
                    },
                })
        })
    }
}
