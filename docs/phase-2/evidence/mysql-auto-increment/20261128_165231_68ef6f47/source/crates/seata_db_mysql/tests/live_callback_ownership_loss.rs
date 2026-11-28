//! 来源登记 COMMIT 的阻断、丢确认、延迟确认；真实回调重试，不重新注册或执行业务。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
mod callback_peer;
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "../src/mysql_loopback_fixture.rs"]
#[allow(dead_code)]
mod mysql_loopback_fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/rm_recovery_fixture.rs"]
#[allow(dead_code)]
mod recovery;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;
#[path = "support/undo_rollback_fixture.rs"]
mod rollback_fixture;
use seata_db_mysql::MySqlDataSource;
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResource, ResourceCatalog,
    ResourceManager,
};
use std::time::Duration;
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned MySQL and original packet callback-origin COMMIT observer"]
async fn callback_origin_commit_unknown_stays_pending_and_original_retry_finishes() {
    let mut admin = fixture::reset().await;
    recovery::schema(&mut admin).await;
    sqlx::raw_sql(include_str!(
        "../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let mode = std::env::var("SEATA_ROLLBACK_FAULT").unwrap();
    let port = std::env::var("SEATA_RM_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let source = MySqlDataSource::connect(
        owned_mysql::options(Some(port))
            .operation_timeout(Duration::from_millis(1500))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = recovery::adapter(source.clone());
    let key = adapter.key().clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter).unwrap();
    let store = rm_fixture::journal().await;
    let rm = ResourceManager::builder(
        rm_fixture::settings(listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        store.clone(),
    )
    .callback_timeout(Duration::from_secs(4))
    .build()
    .unwrap();
    let mut socket = callback_peer::registered(&listener, &key).await;
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "callback:origin-fault",
            9007199254740995,
            -111,
            None,
            BranchPhase::Rollback
        )
        .await,
        (
            ResultCode::Failed,
            BranchStatus::PhaseTwoRollbackFailedRetryable
        )
    );
    let identity = BranchIdentity::new(
        "callback:origin-fault".into(),
        9007199254740995,
        key.clone(),
        None,
    )
    .unwrap();
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    let before = recovery::state(&mut admin).await;
    assert_eq!(
        before,
        if mode == "block_commit" {
            (0, 0, 0)
        } else {
            (1, 2, 0)
        }
    );
    assert_eq!(
        fixture::count(&mut admin, "undo_log").await,
        0,
        "unknown source commit must not begin Undo fencing"
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "callback:origin-fault",
            9007199254740995,
            -112,
            None,
            BranchPhase::Rollback
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    assert_eq!(
        store
            .lease(identity, BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    let after = recovery::state(&mut admin).await;
    assert_eq!(after, (1, 2, 1));
    let origin: (u16, u8, Option<Vec<u8>>) = sqlx::query_as(
        "SELECT schema_version,registration_origin,lock_key FROM seata_rs_at_branch_v1",
    )
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(origin, (2, 2, None));
    assert_eq!(sqlx::query_scalar::<_,i32>("SELECT log_status FROM undo_log WHERE xid='callback:origin-fault' AND branch_id=9007199254740995").fetch_one(&mut admin).await.unwrap(),1);
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "callback:origin-fault",
            9007199254740995,
            -113,
            None,
            BranchPhase::Rollback
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "callback:origin-fault",
            9007199254740995,
            -114,
            None,
            BranchPhase::Commit
        )
        .await
        .0,
        ResultCode::Failed
    );
    assert_eq!(recovery::state(&mut admin).await, after);
    fixture::save(
        "callback-origin-wire-observation.json",
        &serde_json::json!({"mode":mode,"before":before,"after":after,"xid":"callback:origin-fault","branch_id":9007199254740995_i64,"resource_id":key.resource_id(),"first":"Pending","recovered":"Completed","original_register_requests":0,"business_replayed":false,"version":2,"origin":2,"lock_key":null}),
    );
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn compensation_acquisition_rejection_preserves_normal_until_tc_retries() {
    tokio::time::timeout(Duration::from_secs(45), async {
        let (mut admin, seed_source) = rollback_fixture::seed(
            &[],
            &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
            seata_at::MySqlJdbcProfile::Connector8Default,
            None,
        )
        .await;
        seed_source.close().await.unwrap();
        recovery::schema(&mut admin).await;
        sqlx::raw_sql(include_str!(
            "../migrations/002_seata_rs_at_registration_origin.sql"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        let original_undo = rollback_fixture::snapshot(&mut admin).await;
        let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_port = proxy.local_addr().unwrap().port();
        let upstream: u16 = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
        let fault_peer = tokio::spawn(reject_first_compensation_acquisition(proxy, upstream));
        let source = MySqlDataSource::connect(
            owned_mysql::options(Some(proxy_port))
                .operation_timeout(Duration::from_secs(8))
                .unwrap(),
        )
        .await
        .unwrap();
        let adapter = recovery::adapter(source.clone());
        let key = adapter.key().clone();
        let identity = BranchIdentity::new("rollback:xid".into(), 42, key.clone(), None).unwrap();
        let catalog = ResourceCatalog::new(4, 32767).unwrap();
        catalog.register(adapter).unwrap();
        let store = rm_fixture::journal().await;
        let tc = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rm = ResourceManager::builder(
            rm_fixture::settings(tc.local_addr().unwrap()),
            rm_fixture::registration(),
            catalog,
            store.clone(),
        )
        .callback_timeout(Duration::from_secs(20))
        .build()
        .unwrap();
        let mut socket = callback_peer::registered(&tc, &key).await;
        let result = callback_peer::callback(
            &mut socket,
            &key,
            identity.xid(),
            identity.branch_id(),
            -401,
            None,
            BranchPhase::Rollback,
        )
        .await;
        assert_eq!(
            result,
            (
                ResultCode::Success,
                BranchStatus::PhaseTwoRollbackFailedRetryable
            )
        );
        assert_eq!(recovery::state(&mut admin).await, (1, 2, 0));
        assert_eq!(rollback_fixture::snapshot(&mut admin).await, original_undo);
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
        assert_eq!(
            store
                .lease(identity.clone(), BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Pending),
        );

        // 第二次动作由显式 TC 回调触发，使用原身份与已经持久确认的方向。
        let result = callback_peer::callback(
            &mut socket,
            &key,
            identity.xid(),
            identity.branch_id(),
            -402,
            None,
            BranchPhase::Rollback,
        )
        .await;
        assert_eq!(
            result,
            (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
        );
        assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
        assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
        assert_eq!(
            store
                .lease(identity, BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed),
        );
        rm.shutdown().await.unwrap();
        rm_fixture::peer::eof(&mut socket).await;
        source.close().await.unwrap();
        fault_peer.await.unwrap();
        fixture::save(
            "managed-acquisition-after-bind.json",
            &serde_json::json!({
                "xid":"rollback:xid","branch_id":42,"fault_connection":4,
                "first":{"status":9,"phase":2,"terminal":0,"business_rows":1,"undo_preserved":true},
                "retry":{"status":8,"phase":2,"terminal":1,"business_rows":0,"undo_rows":0}
            }),
        );
    })
    .await
    .unwrap();
}

async fn reject_first_compensation_acquisition(listener: TcpListener, upstream: u16) {
    let mut peers = tokio::task::JoinSet::new();
    // 源会话核验、观察、绑定、补偿；复用原分支的第二次回调再走三次控制获取。
    for ordinal in 1..=7 {
        let (mut client, _) = listener.accept().await.unwrap();
        if ordinal == 4 {
            mysql_loopback_fixture::reject_authentication(&mut client, ordinal)
                .await
                .unwrap();
            assert!(mysql_loopback_fixture::read_command(&mut client)
                .await
                .unwrap()
                .is_none());
        } else {
            peers.spawn(async move {
                let mut server = tokio::net::TcpStream::connect(("127.0.0.1", upstream))
                    .await
                    .unwrap();
                // 直接透传协议和连接结束，不记录业务参数或认证内容。
                let result = tokio::io::copy_bidirectional(&mut client, &mut server).await;
                if let Err(error) = result {
                    assert!(
                        matches!(
                            error.kind(),
                            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                        ),
                        "{error}"
                    );
                }
            });
        }
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(75), listener.accept())
            .await
            .is_err()
    );
    while let Some(done) = peers.join_next().await {
        done.unwrap();
    }
}
