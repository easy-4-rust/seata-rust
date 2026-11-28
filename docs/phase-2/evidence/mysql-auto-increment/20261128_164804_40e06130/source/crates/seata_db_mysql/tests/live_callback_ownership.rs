//! 正式连接回调与真实数据库的登记交接；受控 TCP 对端不冒充原 Java TC。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
mod callback_peer;
#[path = "live_rm_recovery.rs"]
mod legacy_recovery;
#[path = "support/callback_migration_cases.rs"]
mod migration;
use legacy_recovery::{fixture, owned_mysql, recovery, rm_fixture};
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::MySqlDataSource;
use seata_protocol::protocol::RegisterRMResponse;
use seata_protocol::transaction::{BranchRegisterRequest, BranchRollbackRequest};
use seata_protocol::{BranchStatus, Message, RequestType, ResultCode};
use seata_rm::{BranchPhase, BranchRecordState, BranchResource, ResourceCatalog, ResourceManager};
use std::time::Duration;
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn manual_dispatch_cannot_mint_callback_ownership() {
    let (mut admin, adapter) = recovery::empty().await;
    sqlx::raw_sql(include_str!(
        "../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let store = rm_fixture::journal().await;
    let dispatcher = seata_rm::BranchDispatcher::new(catalog, store.clone());
    let identity =
        seata_rm::BranchIdentity::new("callback:manual".into(), 42, adapter.key().clone(), None)
            .unwrap();
    for _ in 0..2 {
        assert_eq!(
            dispatcher
                .dispatch(identity.clone(), BranchPhase::Rollback)
                .await
                .unwrap(),
            BranchStatus::PhaseTwoRollbackFailedRetryable
        );
        assert_eq!(recovery::state(&mut admin).await, (0, 0, 0));
        assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
        assert_eq!(
            store
                .lease(identity.clone(), BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Pending)
        );
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn callback_before_positive_response_prevents_late_none_lock_key_enrollment() {
    use seata_db_mysql::{DbError, MySqlRecoveryError};
    use seata_protocol::transaction::BranchRegisterResponse;
    let (mut admin, adapter) = recovery::empty().await;
    sqlx::raw_sql(include_str!(
        "../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        rm_fixture::settings(listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        rm_fixture::journal().await,
    )
    .request_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let key = adapter.key().clone();
    let server_key = key.clone();
    let server = tokio::spawn(async move {
        let mut socket = callback_peer::registered(&listener, &server_key).await;
        let request = rm_fixture::peer::read(&mut socket).await;
        match &request.body {
            Some(Message::BranchRegisterRequest(v)) => {
                assert_eq!(v.xid.as_deref(), Some("callback:early"));
                assert_eq!(v.lock_key, None);
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(
            callback_peer::callback(
                &mut socket,
                &server_key,
                "callback:early",
                42,
                -81,
                None,
                BranchPhase::Rollback
            )
            .await,
            (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
        );
        // 成功响应确实来自原请求，但二阶段已经先获得归属，不能再授权一阶段提交。
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                request.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: rm_fixture::peer::success(),
                    branch_id: 42,
                }),
            ),
        )
        .await;
        let known = rm_fixture::peer::read(&mut socket).await;
        match &known.body {
            Some(Message::BranchRegisterRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("callback:known-none"));
                assert_eq!(value.lock_key, None);
            }
            other => panic!("{other:?}"),
        }
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                known.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: rm_fixture::peer::success(),
                    branch_id: 43,
                }),
            ),
        )
        .await;
        rm_fixture::peer::eof(&mut socket).await;
    });
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize(
            "INSERT INTO undo_work(id,v) VALUES(1,'still_uncommitted')",
        )
        .unwrap(),
        &[],
    )
    .await
    .unwrap();
    let receipt = rm
        .client()
        .branch_register_with_receipt(BranchRegisterRequest {
            xid: Some("callback:early".into()),
            branch_type: key.branch_type(),
            resource_id: Some(key.resource_id().into()),
            application_data: None,
            lock_key: None,
        })
        .await
        .unwrap();
    assert_eq!(receipt.lock_key(), None);
    assert!(
        matches!(
            adapter.enroll(&receipt).await,
            Err(MySqlRecoveryError::BeforeCommit {
                source: DbError::CallbackObserved,
                ..
            })
        ),
        "a genuine late receipt with None must not overwrite unknown callback lock knowledge"
    );
    tx.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    let facts:(u16,u8,u8,u8,Option<Vec<u8>>)=sqlx::query_as("SELECT schema_version,registration_origin,phase,terminal,lock_key FROM seata_rs_at_branch_v1").fetch_one(&mut admin).await.unwrap();
    assert_eq!(facts, (2, 2, 2, 1, None));
    // 同一实际连接另取正常成功回执；已知 None 必须独立保存为 origin1，不能误拒为回调未知。
    let known = rm
        .client()
        .branch_register_with_receipt(BranchRegisterRequest {
            xid: Some("callback:known-none".into()),
            branch_type: receipt.identity().resource().branch_type(),
            resource_id: Some(receipt.identity().resource().resource_id().into()),
            application_data: None,
            lock_key: None,
        })
        .await
        .unwrap();
    adapter.enroll(&known).await.unwrap();
    adapter.enroll(&known).await.unwrap();
    let known_facts:(u16,u8,u8,u8,Option<Vec<u8>>)=sqlx::query_as("SELECT schema_version,registration_origin,phase,terminal,lock_key FROM seata_rs_at_branch_v1 WHERE xid=?").bind(b"callback:known-none".as_slice()).fetch_one(&mut admin).await.unwrap();
    assert_eq!(known_facts, (2, 1, 0, 0, None));
    fixture::save(
        "callback-early-positive.json",
        &serde_json::json!({"genuine_positive_receipt":true,"lock_key":null,"origin":2,"phase":2,"terminal":1,"late_enroll":"CallbackObserved","business_rows":0,"known_None_positive_origin":1,"known_None_positive_phase":0,"known_None_duplicate_enrollment":"accepted"}),
    );
    rm.shutdown().await.unwrap();
    server.await.unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn expanded_schema_preserves_known_receipt_and_enrollment_remains_idempotent() {
    let (mut admin, adapter) = recovery::empty().await;
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    type RecoverySnapshot = (
        u16,
        Vec<u8>,
        i64,
        Vec<u8>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        u8,
        u8,
        u8,
        String,
        String,
    );
    let original: RecoverySnapshot=sqlx::query_as("SELECT schema_version,xid,branch_id,resource_id,application_data,lock_key,jdbc_profile,phase,terminal,CAST(created_at AS CHAR),CAST(modified_at AS CHAR) FROM seata_rs_at_branch_v1").fetch_one(&mut admin).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    adapter
        .enroll(&receipt)
        .await
        .expect("expanded schema must preserve valid original registration");
    let retained=sqlx::query_as::<_,RecoverySnapshot>("SELECT schema_version,xid,branch_id,resource_id,application_data,lock_key,jdbc_profile,phase,terminal,CAST(created_at AS CHAR),CAST(modified_at AS CHAR) FROM seata_rs_at_branch_v1").fetch_one(&mut admin).await.unwrap();
    assert_eq!(retained, original);
    assert_eq!(
        sqlx::query_scalar::<_, u8>("SELECT registration_origin FROM seata_rs_at_branch_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn lost_registration_response_is_handed_to_validated_callback_without_forged_success_receipt()
{
    let (mut admin, adapter) = recovery::empty().await;
    sqlx::raw_sql(include_str!(
        "../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let store = rm_fixture::journal().await;
    let saved = store.clone();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let key = adapter.key().clone();
    let expected_key = key.clone();
    let rm = ResourceManager::builder(
        rm_fixture::settings(listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        store,
    )
    .request_timeout(Duration::from_millis(400))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let (ready, proceed) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let register = rm_fixture::peer::read(&mut socket).await;
        match register.body {
            Some(Message::RegisterRMRequest(v)) => {
                assert_eq!(v.resource_ids.as_deref(), Some(expected_key.resource_id()))
            }
            other => panic!("{other:?}"),
        };
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                register.id,
                Message::RegisterRMResponse(RegisterRMResponse {
                    identified: true,
                    wire_version: None,
                }),
            ),
        )
        .await;
        let register = rm_fixture::peer::read(&mut socket).await;
        match register.body {
            Some(Message::BranchRegisterRequest(v)) => {
                assert_eq!(v.xid.as_deref(), Some("callback:lost"));
                assert_eq!(v.lock_key.as_deref(), Some("undo_work:1"));
                assert_eq!(v.resource_id.as_deref(), Some(expected_key.resource_id()));
            }
            other => panic!("{other:?}"),
        };
        // TC 模型已分配 9007199254740995，但故意丢弃原成功响应；原连接仍可下发回调。
        proceed.await.unwrap();
        let mut callback = rm_fixture::peer::reply(
            -71,
            Message::BranchRollbackRequest(BranchRollbackRequest {
                xid: "callback:lost".into(),
                branch_id: 9007199254740995,
                branch_type: expected_key.branch_type(),
                resource_id: expected_key.resource_id().into(),
                application_data: None,
            }),
        );
        callback.message_type = RequestType::RequestSync;
        rm_fixture::peer::write(&mut socket, callback).await;
        let response = rm_fixture::peer::read(&mut socket).await;
        assert_eq!(response.id, -71);
        match response.body {
            Some(Message::BranchRollbackResponse(v)) => {
                assert_eq!(v.response.result_code, ResultCode::Success);
                assert_eq!(v.branch_status,BranchStatus::PhaseTwoRollbacked,"validated callback must recover lost registration without a fabricated success receipt");
                assert_eq!(v.branch_id, 9007199254740995);
                assert_eq!(v.xid.as_deref(), Some("callback:lost"));
            }
            other => panic!("{other:?}"),
        };
        rm_fixture::peer::eof(&mut socket).await;
    });
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'not_committed')")
            .unwrap(),
        &[],
    )
    .await
    .unwrap();
    let error = rm
        .client()
        .branch_register_with_receipt(BranchRegisterRequest {
            xid: Some("callback:lost".into()),
            branch_type: key.branch_type(),
            resource_id: Some(key.resource_id().into()),
            application_data: None,
            lock_key: Some("undo_work:1".into()),
        })
        .await
        .unwrap_err();
    assert!(error.is_unknown_outcome());
    tx.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    ready.send(()).unwrap();
    // 让回调完成再关闭 RM，不能因关闭本身导致测试仅检查到 EOF。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
    let id =
        seata_rm::BranchIdentity::new("callback:lost".into(), 9007199254740995, key, None).unwrap();
    loop {
        let lease = saved
            .lease(id.clone(), BranchPhase::Rollback)
            .await
            .unwrap();
        let done = lease.state() == Some(BranchRecordState::Completed);
        drop(lease);
        if done {
            break;
        }
        if server.is_finished() {
            server.await.unwrap();
            panic!("callback did not publish Completed");
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "callback completion deadline"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let facts:(u16,u8,u8,u8,Option<Vec<u8>>)=sqlx::query_as("SELECT schema_version,registration_origin,phase,terminal,lock_key FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?").bind(b"callback:lost".as_slice()).bind(9007199254740995_i64).fetch_one(&mut admin).await.unwrap();
    assert_eq!(facts, (2, 2, 2, 1, None));
    assert_eq!(sqlx::query_scalar::<_,i32>("SELECT log_status FROM undo_log WHERE xid='callback:lost' AND branch_id=9007199254740995").fetch_one(&mut admin).await.unwrap(),1);
    fixture::save(
        "callback-lost-registration.json",
        &serde_json::json!({"xid":"callback:lost","branch_id":9007199254740995_i64,"phase":2,"origin":2,"terminal":1,"business_rows":fixture::count(&mut admin,"undo_work").await,"original_register_requests":1,"fabricated_registered_receipt":false}),
    );
    rm.shutdown().await.unwrap();
    server.await.unwrap();
    ds.close().await.unwrap();
}
