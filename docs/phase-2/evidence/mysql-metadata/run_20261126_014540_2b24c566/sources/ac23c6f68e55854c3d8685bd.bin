//! 固定 Java TC 下，同一分支跨行和跨表申请多个排他锁。
use super::{fixture, lock_guard_cases::checkpoint};
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::transaction::GlobalLockQueryRequest;
use seata_protocol::{BranchType, GlobalStatus, Message, TransactionExceptionCode};
use seata_rm::{BranchResource, ResourceManager, RmCallError};
use seata_tm::{TransactionConfig, TransactionManager};
use sqlx::MySqlConnection;
use std::{path::Path, time::Duration};
use tokio::time::{sleep, Instant};

const ENTIRE_BUNDLE: &str = "accounts:0,1;reserves:1";
const FREE_SUBSET: &str = "accounts:0;reserves:1";

/// 先确认冲突登记的原回包，再分别验证空闲子集及释放后的完整申请。
pub(super) async fn exercise(
    admin: &mut MySqlConnection,
    source: &MySqlDataSource,
    manager: &MySqlDataSourceManager,
    rm: &ResourceManager,
    tm: &TransactionManager,
    folder: &Path,
) {
    let config = TransactionConfig::builder("java-rust-lock-bundle".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let client = rm.client();
    let resource = manager.key().resource_id();
    let accounts =
        SqlRecognizerFactory::recognize("UPDATE accounts SET balance=balance-10 WHERE id IN (0,1)")
            .unwrap();
    let free_account =
        SqlRecognizerFactory::recognize("UPDATE accounts SET balance=balance-10 WHERE id=0")
            .unwrap();
    let reserve =
        SqlRecognizerFactory::recognize("UPDATE reserves SET balance=balance-10 WHERE id=1")
            .unwrap();

    let mut rejected = tm.begin(&config).await.unwrap();
    let rejected_xid = rejected.xid().clone();
    let mut local = source.begin().await.unwrap();
    assert_eq!(local.execute(&accounts, &[]).await.unwrap(), 2);
    assert_eq!(local.execute(&reserve, &[]).await.unwrap(), 1);
    let error = manager
        .prepare_transaction(local, &client, rejected_xid.as_str(), None)
        .await
        .unwrap_err();
    let MySqlPrepareError::Registration(RmCallError::DefiniteRejected { response, .. }) = error
    else {
        panic!("expected original TC lock rejection: {error:?}");
    };
    let Some(Message::BranchRegisterResponse(rejection)) = response.body else {
        panic!("expected original BranchRegisterResponse");
    };
    let reason = rejection.response.transaction_exception_code;
    assert!(matches!(
        reason,
        TransactionExceptionCode::LockKeyConflict
            | TransactionExceptionCode::LockKeyConflictFailFast
    ));
    // 先从独立连接确认拒绝后没有部分锁，再结束失败方的原全局事务。
    checkpoint(
        folder,
        "bundle-rejected",
        &serde_json::json!({"xid":rejected_xid.as_str(),"resource":resource,
           "lock_key":ENTIRE_BUNDLE,"error":reason.to_java_v1()}),
    )
    .await;
    rejected.rollback().await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while tm.get_status(&rejected_xid).await.unwrap().status() != GlobalStatus::Finished {
        assert!(Instant::now() < deadline, "rejected bundle remains active");
        sleep(Duration::from_millis(40)).await;
    }

    for (statement, lock_key, expected_rows, stage) in [
        (&free_account, FREE_SUBSET, 1, "free-subset-admitted"),
        (&accounts, ENTIRE_BUNDLE, 2, "whole-bundle-admitted"),
    ] {
        let mut admitted = tm.begin(&config).await.unwrap();
        let xid = admitted.xid().clone();
        assert!(
            client
                .lock_query(GlobalLockQueryRequest {
                    xid: Some(xid.as_str().into()),
                    branch_type: BranchType::At,
                    resource_id: Some(resource.into()),
                    lock_key: Some(lock_key.into()),
                    application_data: None,
                })
                .await
                .unwrap()
                .lockable
        );
        let mut local = source.begin().await.unwrap();
        assert_eq!(local.execute(statement, &[]).await.unwrap(), expected_rows);
        assert_eq!(local.execute(&reserve, &[]).await.unwrap(), 1);
        let prepared = manager
            .prepare_transaction(local, &client, xid.as_str(), None)
            .await
            .unwrap();
        let receipt = prepared.registered_branch().unwrap();
        assert_eq!(receipt.lock_key(), Some(lock_key));
        let branch = receipt.identity().branch_id();
        prepared.rollback().await.unwrap();
        let state = serde_json::json!({"xid":xid.as_str(),"resource":resource,
            "lock_key":lock_key,"branch_id":branch});
        checkpoint(folder, stage, &state).await;
        admitted.rollback().await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
            assert!(Instant::now() < deadline, "admitted bundle remains active");
            sleep(Duration::from_millis(40)).await;
        }
        let terminal: (u8, u8, u8) = sqlx::query_as("SELECT phase,terminal,first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?")
            .bind(xid.as_str().as_bytes()).bind(branch).fetch_one(&mut *admin).await.unwrap();
        assert_eq!(terminal, (2, 1, 0));
        if stage == "free-subset-admitted" {
            checkpoint(folder, "free-subset-released", &state).await;
        } else {
            fixture::save(
                "tc-lock-guard-result.json",
                &serde_json::json!({
                    "rejected_xid":rejected_xid.as_str(),"final_branch":state,"recovery_state":terminal
                }),
            );
        }
    }
}
