//! 一阶段准备只交接原事务与真实 TC 回执；没有业务 COMMIT 入口。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
#[allow(dead_code)]
mod callback_peer;
#[path = "support/first_phase_cases.rs"]
mod cases;
#[path = "support/first_phase_fixture.rs"]
mod first_fixture;
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/rm_recovery_fixture.rs"]
#[allow(dead_code)]
mod recovery;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;
#[path = "support/first_phase_wait_cases.rs"]
mod wait_cases;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_protocol::transaction::BranchRegisterResponse;
use seata_protocol::Message;
use seata_rm::{BranchResource, ResourceCatalog, ResourceManager};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn preparation_uses_real_images_and_positive_receipt_but_keeps_business_and_undo_uncommitted()
{
    let (mut admin, _) = recovery::empty().await;
    sqlx::raw_sql(include_str!(
        "../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let key = adapter.key().clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        rm_fixture::settings(listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        rm_fixture::journal().await,
    )
    .request_timeout(Duration::from_secs(2))
    .build()
    .unwrap();
    let server_key = key.clone();
    let server = tokio::spawn(async move {
        let mut socket = callback_peer::registered(&listener, &server_key).await;
        let request = rm_fixture::peer::read(&mut socket).await;
        match request.body {
            Some(Message::BranchRegisterRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("prepare:positive"));
                assert_eq!(value.resource_id.as_deref(), Some(server_key.resource_id()));
                assert_eq!(value.lock_key.as_deref(), Some("undo_work:1"));
                assert_eq!(
                    value.application_data.as_deref(),
                    Some("original-prepare-data")
                );
            }
            other => panic!("{other:?}"),
        };
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                request.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: rm_fixture::peer::success(),
                    branch_id: 9007199254740995,
                }),
            ),
        )
        .await;
        rm_fixture::peer::eof(&mut socket).await;
    });
    let mut transaction = source.begin().await.unwrap();
    transaction
        .execute(
            &SqlRecognizerFactory::recognize(
                "INSERT INTO undo_work(id,v) VALUES(1,'prepare_not_committed')",
            )
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
    let prepared = adapter
        .prepare_transaction(
            transaction,
            &rm.client(),
            "prepare:positive",
            Some("original-prepare-data".into()),
        )
        .await
        .expect("the prepared transaction must retain its genuine registered branch");
    let receipt = prepared.registered_branch().unwrap();
    assert_eq!(receipt.identity().branch_id(), 9007199254740995);
    assert_eq!(receipt.identity().resource(), &key);
    assert_eq!(receipt.lock_key(), Some("undo_work:1"));
    assert_eq!(prepared.undo_log_count(), 1);
    // 独立脏读仅用于证明准备已实际写入；随后恢复隔离，不把未提交可见性当提交证明。
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL READ UNCOMMITTED")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    let dirty: (i64, i32) =
        sqlx::query_as("SELECT branch_id,log_status FROM undo_log WHERE xid='prepare:positive'")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!(dirty, (9007199254740995, 0));
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
    prepared.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
    fixture::save(
        "first-phase-positive.json",
        &serde_json::json!({"xid":"prepare:positive","branch_id":9007199254740995_i64,"resource_id":key.resource_id(),"lock_key":"undo_work:1","registered_requests":1,"business_rows_after_rollback":0,"undo_rows_after_rollback":0,"control_rows_after_rollback":1,"business_COMMIT":false}),
    );
    rm.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn empty_and_zero_row_changes_do_not_register_or_create_recovery_ownership() {
    let (mut admin, _) = recovery::empty().await;
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let key = adapter.key().clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        rm_fixture::settings(listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        rm_fixture::journal().await,
    )
    .build()
    .unwrap();
    let server = tokio::spawn(async move {
        let mut socket = callback_peer::registered(&listener, &key).await;
        rm_fixture::peer::eof(&mut socket).await;
    });
    for empty in [true, false] {
        let mut transaction = source.begin().await.unwrap();
        if !empty {
            assert_eq!(
                transaction
                    .execute(
                        &SqlRecognizerFactory::recognize(
                            "UPDATE undo_work SET v='no_row' WHERE id=1"
                        )
                        .unwrap(),
                        &[]
                    )
                    .await
                    .unwrap(),
                0
            );
        }
        let prepared = adapter
            .prepare_transaction(transaction, &rm.client(), "prepare:empty", None)
            .await
            .unwrap();
        assert!(prepared.registered_branch().is_none());
        assert_eq!(prepared.undo_log_count(), 0);
        prepared.rollback().await.unwrap();
    }
    assert_eq!(recovery::state(&mut admin).await, (0, 0, 0));
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    rm.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}
