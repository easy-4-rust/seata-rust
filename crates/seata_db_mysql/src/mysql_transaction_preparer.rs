use crate::{
    DbError, MySqlDataSourceManager, MySqlLocalTransaction, MySqlPrepareError,
    MySqlPreparedTransaction,
};
use seata_at::MySqlJdbcProfile;
use seata_protocol::transaction::BranchRegisterRequest;
use seata_rm::{BranchResource, ResourceClient};
use tokio::time::{timeout_at, Instant};

/// 消耗本地作用域的有界登记／Undo 准备编排；不提交业务、不重试注册或重放 SQL。
/// 对应 Java: org.apache.seata.rm.datasource.ConnectionProxy#register 与 processGlobalTransactionCommit 的准备部分。
pub(crate) struct MySqlTransactionPreparer;
impl MySqlTransactionPreparer {
    /// 参数为精确资源管理器、原事务和原注册字段；返回仅可回滚的准备作用域。
    pub(crate) async fn prepare(
        manager: &MySqlDataSourceManager,
        transaction: MySqlLocalTransaction,
        client: &ResourceClient,
        xid: &str,
        application_data: Option<String>,
        profile: MySqlJdbcProfile,
        failure_slot: Option<&mut Option<MySqlPrepareError>>,
    ) -> Result<MySqlPreparedTransaction, MySqlPrepareError> {
        let guard = manager
            .phase_one_metrics_sampler()
            .begin(crate::MySqlPhaseOneOperation::Prepare);
        let result = Self::prepare_inner(
            manager,
            transaction,
            client,
            xid,
            application_data,
            profile,
            failure_slot,
        )
        .await;
        guard.finish_prepare(&result);
        result
    }

    async fn prepare_inner(
        manager: &MySqlDataSourceManager,
        mut transaction: MySqlLocalTransaction,
        client: &ResourceClient,
        xid: &str,
        application_data: Option<String>,
        profile: MySqlJdbcProfile,
        mut failure_slot: Option<&mut Option<MySqlPrepareError>>,
    ) -> Result<MySqlPreparedTransaction, MySqlPrepareError> {
        let deadline = Instant::now() + manager.operation_timeout();
        let mut known = None;
        let mut registration_started = false;
        let result = timeout_at(deadline, async {
            if xid.is_empty()
                || xid.len() > 128
                || xid.trim() != xid
                || xid.chars().any(char::is_control)
                || application_data
                    .as_ref()
                    .is_some_and(|v| v.is_empty() || v.len() > 1_048_576)
            {
                return Err(MySqlPrepareError::Local {
                    source: DbError::Input {
                        reason: "bounded exact preparation identity required",
                    },
                });
            }
            let lock_key = transaction.preparation_lock_keys(
                manager.key().resource_id(),
                manager.server_uuid(),
                profile,
            )?;
            if transaction.undo_logs().is_empty() {
                return Ok(MySqlPreparedTransaction::new(
                    transaction,
                    manager.source_clone(),
                    None,
                    profile,
                ));
            }
            transaction
                .verify_registration_checks(
                    manager.key().resource_id(),
                    lock_key.as_deref(),
                    profile,
                    manager.registration_checks(),
                )
                .await?;
            if Instant::now() >= deadline {
                return Err(MySqlPrepareError::Deadline {
                    receipt: None,
                    registration_may_have_effect: false,
                });
            }
            registration_started = true;
            if let Some(slot) = failure_slot.as_mut() {
                Self::record_registration_started(slot);
            }
            let receipt = client
                .branch_register_with_receipt(BranchRegisterRequest {
                    xid: Some(xid.into()),
                    branch_type: manager.key().branch_type(),
                    resource_id: Some(manager.key().resource_id().into()),
                    application_data,
                    lock_key,
                })
                .await
                .map_err(MySqlPrepareError::Registration)?;
            known = Some(Box::new(receipt.clone()));
            if let Some(slot) = failure_slot.as_mut() {
                Self::record_registered(slot, &receipt);
            }
            if let Err(source) = manager.enroll(&receipt).await {
                let rollback = Self::rollback_after_registration(transaction, deadline).await;
                return Err(MySqlPrepareError::Enrollment {
                    receipt: Box::new(receipt),
                    source,
                    rollback,
                    report_guard: crate::MySqlFailureReportGuard::new(Some(
                        manager.phase_one_metrics_sampler(),
                    )),
                });
            }
            if let Err(source) = transaction
                .flush_undo_logs(
                    receipt.identity().xid(),
                    receipt.identity().branch_id(),
                    profile,
                )
                .await
            {
                let rollback = Self::rollback_after_registration(transaction, deadline).await;
                return Err(MySqlPrepareError::Undo {
                    receipt: Box::new(receipt),
                    source,
                    rollback,
                    report_guard: crate::MySqlFailureReportGuard::new(Some(
                        manager.phase_one_metrics_sampler(),
                    )),
                });
            }
            Ok(MySqlPreparedTransaction::new(
                transaction,
                manager.source_clone(),
                Some(receipt),
                profile,
            ))
        })
        .await;
        match result {
            Ok(Err(error)) => Err(error),
            Ok(Ok(prepared)) if Instant::now() < deadline => Ok(prepared),
            _ => Err(MySqlPrepareError::Deadline {
                receipt: known,
                registration_may_have_effect: registration_started,
            }),
        }
    }

    /// 参数为调用方错误槽；单次注册等待开始前记下可能已生效，不合成分支编号。
    pub(crate) fn record_registration_started(slot: &mut Option<MySqlPrepareError>) {
        *slot = Some(MySqlPrepareError::Deadline {
            receipt: None,
            registration_may_have_effect: true,
        });
    }

    /// 参数为调用方错误槽和真实注册回执；后续准备等待取消时保留原编号，但不确认本地回滚。
    pub(crate) fn record_registered(
        slot: &mut Option<MySqlPrepareError>,
        receipt: &seata_rm::RegisteredBranch,
    ) {
        *slot = Some(MySqlPrepareError::Deadline {
            receipt: Some(Box::new(receipt.clone())),
            registration_may_have_effect: true,
        });
    }

    /// 参数为已登记分支仍拥有的原业务事务及入口绝对期限；返回显式回滚确认或保守错误。
    /// 只有 Ok(()) 才可作为后续 PhaseOne_Failed 报告的本地事实。
    async fn rollback_after_registration(
        transaction: MySqlLocalTransaction,
        deadline: Instant,
    ) -> Result<(), Box<DbError>> {
        timeout_at(deadline, transaction.rollback())
            .await
            .map_err(|_| {
                Box::new(DbError::Deadline {
                    operation: "preparation rollback",
                })
            })?
            .map_err(Box::new)
    }
}
