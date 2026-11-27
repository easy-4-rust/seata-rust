use crate::{
    mysql_xa_branch_store::MySqlXaBranchStore, DbError, MySqlXaEnlistError, MySqlXaResourceManager,
    MySqlXaTransaction,
};
use seata_at::SqlRecognizer;
use seata_datasource::FieldValue;
use seata_rm::RegisteredBranch;
use tokio::time::{timeout_at, Instant};
/// 原成功登记独占的 ACTIVE 作用域；方向绑定后不得继续业务或 Prepare，不暴露本地 Commit。
/// 对应 Java: ConnectionProxyXA 的登记后业务和 Prepare 生命周期（选定路径）。
#[derive(Debug)]
pub struct MySqlXaEnlistedTransaction {
    manager: MySqlXaResourceManager,
    receipt: RegisteredBranch,
    transaction: Option<MySqlXaTransaction>,
}
impl MySqlXaEnlistedTransaction {
    /// 参数为已持久原登记的管理器、原回执与原活动事务；返回受管理作用域，仅内部构造。
    pub(crate) fn new(
        manager: MySqlXaResourceManager,
        receipt: RegisteredBranch,
        transaction: MySqlXaTransaction,
    ) -> Self {
        Self {
            manager,
            receipt,
            transaction: Some(transaction),
        }
    }
    /// 无参数；返回原正式登记回执借用。消费式 Prepare/取消前调用方应保留原身份。
    pub fn registered_branch(&self) -> &RegisteredBranch {
        &self.receipt
    }
    /// 参数为原支持 DML/参数；持原控制行锁确认未绑定二阶段后执行一次，失败/取消封闭原作用域。
    pub async fn execute(
        &mut self,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<u64, MySqlXaEnlistError> {
        let mut native = self
            .transaction
            .take()
            .ok_or_else(|| MySqlXaEnlistError::Control {
                receipt: Box::new(self.receipt.clone()),
                source: DbError::Poisoned,
            })?;
        let deadline = Instant::now() + self.manager.native.source.operation_timeout();
        let receipt = &self.receipt;
        let result = timeout_at(deadline, async {
            let (gate, row) =
                MySqlXaBranchStore::lock(&self.manager.native.source, receipt.identity())
                    .await
                    .map_err(|source| MySqlXaEnlistError::Control {
                        receipt: Box::new(receipt.clone()),
                        source,
                    })?;
            if !row.is_some_and(|r| r.phase == 0 && !r.completed) {
                return Err(MySqlXaEnlistError::Control {
                    receipt: Box::new(receipt.clone()),
                    source: DbError::RecoveryConflict,
                });
            }
            let count = native.execute(sql, parameters).await.map_err(|source| {
                MySqlXaEnlistError::Native {
                    receipt: Box::new(receipt.clone()),
                    source,
                }
            })?;
            gate.rollback()
                .await
                .map_err(|e| MySqlXaEnlistError::Control {
                    receipt: Box::new(receipt.clone()),
                    source: DbError::driver("XA business gate release", e),
                })?;
            Ok(count)
        })
        .await;
        if Instant::now() >= deadline {
            return Err(MySqlXaEnlistError::Deadline {
                receipt: Some(Box::new(receipt.clone())),
                registration_may_have_effect: true,
            });
        }
        let count = result.map_err(|_| MySqlXaEnlistError::Deadline {
            receipt: Some(Box::new(receipt.clone())),
            registration_may_have_effect: true,
        })??;
        self.transaction = Some(native);
        Ok(count)
    }
    /// 消耗原作用域，持原行锁单次 END/PREPARE；成功返回原真实回执，不自动发送 PhaseOne_Done。
    /// 原生响应未知/取消不重做；错误保留真实回执，Prepared 缺失不能代替已确认回滚。
    pub async fn prepare(mut self) -> Result<RegisteredBranch, MySqlXaEnlistError> {
        let mut native = self
            .transaction
            .take()
            .ok_or_else(|| MySqlXaEnlistError::Control {
                receipt: Box::new(self.receipt.clone()),
                source: DbError::Poisoned,
            })?;
        let deadline = Instant::now() + self.manager.native.source.operation_timeout();
        let receipt = &self.receipt;
        let result = timeout_at(deadline, async {
            let (mut gate, row) =
                MySqlXaBranchStore::lock(&self.manager.native.source, receipt.identity())
                    .await
                    .map_err(|source| MySqlXaEnlistError::Control {
                        receipt: Box::new(receipt.clone()),
                        source,
                    })?;
            if !row.is_some_and(|r| r.phase == 0 && !r.completed) {
                return Err(MySqlXaEnlistError::Control {
                    receipt: Box::new(receipt.clone()),
                    source: DbError::RecoveryConflict,
                });
            }
            crate::mysql_xa_branch_store::MySqlXaBranchStore::validate_table(
                &mut gate,
                &self.manager.native.source,
                crate::mysql_xa_commit_witness::MySqlXaCommitWitness::TABLE,
            )
            .await
            .map_err(|source| MySqlXaEnlistError::Control {
                receipt: Box::new(receipt.clone()),
                source,
            })?;
            native
                .insert_commit_witness(receipt.identity())
                .await
                .map_err(|source| MySqlXaEnlistError::Native {
                    receipt: Box::new(receipt.clone()),
                    source,
                })?;
            crate::mysql_xa_prepare_evidence::MySqlXaPrepareEvidence::stage(
                &mut gate,
                &self.manager.native.source,
                receipt.identity(),
            )
            .await
            .map_err(|source| MySqlXaEnlistError::Control {
                receipt: Box::new(receipt.clone()),
                source,
            })?;
            // 先确认原 XA 已写见证的生产者证据，再重新仲裁方向；未持有新门闩时不发送 END。
            gate.commit()
                .await
                .map_err(|e| MySqlXaEnlistError::Control {
                    receipt: Box::new(receipt.clone()),
                    source: DbError::driver("XA staged evidence commit", e),
                })?;
            let (gate, row) =
                MySqlXaBranchStore::lock(&self.manager.native.source, receipt.identity())
                    .await
                    .map_err(|source| MySqlXaEnlistError::Control {
                        receipt: Box::new(receipt.clone()),
                        source,
                    })?;
            if !row.is_some_and(|r| r.phase == 0 && !r.completed) {
                return Err(MySqlXaEnlistError::Control {
                    receipt: Box::new(receipt.clone()),
                    source: DbError::RecoveryConflict,
                });
            }
            native
                .prepare()
                .await
                .map_err(|source| MySqlXaEnlistError::Native {
                    receipt: Box::new(receipt.clone()),
                    source,
                })?;
            gate.rollback()
                .await
                .map_err(|e| MySqlXaEnlistError::Control {
                    receipt: Box::new(receipt.clone()),
                    source: DbError::driver("XA prepare gate release", e),
                })?;
            Ok(())
        })
        .await;
        if Instant::now() >= deadline {
            return Err(MySqlXaEnlistError::Deadline {
                receipt: Some(Box::new(self.receipt)),
                registration_may_have_effect: true,
            });
        }
        result.map_err(|_| MySqlXaEnlistError::Deadline {
            receipt: Some(Box::new(self.receipt.clone())),
            registration_may_have_effect: true,
        })??;
        Ok(self.receipt)
    }
}
