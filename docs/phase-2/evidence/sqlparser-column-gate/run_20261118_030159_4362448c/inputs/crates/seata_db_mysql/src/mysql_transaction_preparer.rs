use crate::{
    DbError, MySqlDataSourceManager, MySqlLocalTransaction, MySqlPrepareError,
    MySqlPreparedTransaction,
};
use seata_at::MySqlJdbcProfile;
use seata_protocol::transaction::BranchRegisterRequest;
use seata_rm::{BranchResource, ResourceClient};
use std::time::Duration;
use tokio::time::{timeout_at, Instant};

/// 消耗本地作用域的有界登记／Undo 准备编排；不提交业务、不重试注册或重放 SQL。
/// 对应 Java: org.apache.seata.rm.datasource.ConnectionProxy#register 与 processGlobalTransactionCommit 的准备部分。
pub(crate) struct MySqlTransactionPreparer;
impl MySqlTransactionPreparer {
    /// 参数为精确资源管理器、原事务和原注册字段；返回仅可回滚的准备作用域。
    pub(crate) async fn prepare(
        manager: &MySqlDataSourceManager,
        mut transaction: MySqlLocalTransaction,
        client: &ResourceClient,
        xid: &str,
        application_data: Option<String>,
        profile: MySqlJdbcProfile,
        budget: Duration,
    ) -> Result<MySqlPreparedTransaction, MySqlPrepareError> {
        let deadline = Instant::now() + budget;
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
            let lock_key =
                transaction.preparation_lock_keys(manager.key().resource_id(), profile)?;
            if transaction.undo_logs().is_empty() {
                return Ok(MySqlPreparedTransaction::new(transaction, None, profile));
            }
            registration_started = true;
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
            manager
                .enroll(&receipt)
                .await
                .map_err(|source| MySqlPrepareError::Enrollment {
                    receipt: Box::new(receipt.clone()),
                    source,
                })?;
            transaction
                .flush_undo_logs(
                    receipt.identity().xid(),
                    receipt.identity().branch_id(),
                    profile,
                )
                .await
                .map_err(|source| MySqlPrepareError::Undo {
                    receipt: Box::new(receipt.clone()),
                    source,
                })?;
            Ok(MySqlPreparedTransaction::new(
                transaction,
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
}
