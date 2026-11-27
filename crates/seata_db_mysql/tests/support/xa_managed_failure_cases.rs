//! 真实成功响应丢失及早于登记响应的回调；不得把暂时缺失误报为终态。
#![allow(clippy::unwrap_used)]
#[path = "xa_command_proxy.rs"]
mod xa_command_proxy;
use super::{bootstrap, callback, fixture, peer, registered, rm};
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::{MySqlDataSource, MySqlXaResourceManager};
use seata_protocol::{transaction::BranchRegisterResponse, BranchType, Message, ResultCode};
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResource};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned real MySQL and original XA COMMIT OK loss"]
async fn lost_native_commit_ok_preserves_rm_pending_and_does_not_replay() {
    let (mut admin, plain, _) = fixture::seed().await;
    let original = seata_xa::XaBranchXid::new(Some("xa:managed-loss".into()), 9007199254741001);
    let mut command = vec![3];
    command
        .extend_from_slice(format!("XA COMMIT {}", original.mysql_arguments().unwrap()).as_bytes());
    let proxy = xa_command_proxy::XaCommandProxy::start(
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        command.clone(),
    )
    .await;
    let source = MySqlDataSource::connect(
        fixture::owned_mysql::options(Some(proxy.port))
            .operation_timeout(Duration::from_millis(750))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = Arc::new(MySqlXaResourceManager::new(source.clone()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, store) = rm(adapter.clone(), &listener, "native-loss").await;
    let key = adapter.key().clone();
    let identity = BranchIdentity::new(
        "xa:managed-loss".into(),
        9007199254741001,
        key.clone(),
        None,
    )
    .unwrap();
    let peer_identity = identity.clone();
    let (start, ready) = tokio::sync::oneshot::channel();
    let (done, finished) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = bootstrap(&listener, key.resource_id()).await;
        registered(
            &mut socket,
            key.resource_id(),
            "xa:managed-loss",
            None,
            9007199254741001,
        )
        .await;
        ready.await.unwrap();
        assert_eq!(
            callback(&mut socket, -27, &peer_identity, BranchPhase::Commit)
                .await
                .1,
            6
        );
        assert_eq!(
            callback(&mut socket, -28, &peer_identity, BranchPhase::Commit)
                .await
                .1,
            6
        );
        done.send(()).unwrap();
        peer::eof(&mut socket).await;
    });
    let mut tx = adapter
        .register_and_start(&manager.client(), identity.xid(), None)
        .await
        .unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(1,42)").unwrap(),
        &[],
    )
    .await
    .unwrap();
    tx.prepare().await.unwrap();
    proxy.arm();
    start.send(()).unwrap();
    finished.await.unwrap();
    let (request, response) = proxy.captured().unwrap();
    assert_eq!(request[4..], command);
    assert_eq!(response[4], 0);
    assert_eq!(fixture::business(&mut admin).await, vec![(1, 42)]);
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending),
        "lost native success was falsely completed in RM journal"
    );
    let row: (u8, u8) = sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_branch_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(row, (1, 0));
    assert!(sqlx::raw_sql("XA RECOVER")
        .fetch_all(&mut admin)
        .await
        .unwrap()
        .is_empty());
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    let count = proxy.shutdown().await;
    assert_eq!(
        count, 1,
        "native COMMIT was blindly replayed after missing Prepared"
    );
    fixture::record(
        "native-commit-loss",
        serde_json::json!({"branch_id":identity.branch_id().to_string(),"xid":identity.xid(),"request":request,"held_original_ok":response,"actual_commands":count,"callback_statuses":[6,6],"journal":"Pending","control_state":[1,0],"business":[[1,42]],"missing_prepared_not_success":true}),
    );
    source.close().await.unwrap();
    plain.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL and early original registration callback"]
async fn callback_before_registration_reply_stays_pending_until_original_prepare() {
    let (mut admin, source, adapter) = fixture::seed().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, store) = rm(adapter.clone(), &listener, "early").await;
    let key = adapter.key().clone();
    let identity =
        BranchIdentity::new("xa:early".into(), 9007199254741002, key.clone(), None).unwrap();
    let peer_identity = identity.clone();
    let (start, ready) = tokio::sync::oneshot::channel();
    let (done, finished) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = bootstrap(&listener, key.resource_id()).await;
        let request = peer::read(&mut socket).await;
        assert!(
            matches!(request.body,Some(Message::BranchRegisterRequest(ref v)) if v.branch_type==BranchType::Xa&&v.lock_key.is_none())
        );
        assert_eq!(
            callback(&mut socket, -31, &peer_identity, BranchPhase::Rollback).await,
            (ResultCode::Success, 9),
            "early rollback falsely confirmed missing registered branch"
        );
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: peer::success(),
                    branch_id: peer_identity.branch_id(),
                }),
            ),
        )
        .await;
        ready.await.unwrap();
        assert_eq!(
            callback(&mut socket, -32, &peer_identity, BranchPhase::Rollback).await,
            (ResultCode::Success, 8)
        );
        done.send(()).unwrap();
        peer::eof(&mut socket).await;
    });
    let mut tx = adapter
        .register_and_start(&manager.client(), identity.xid(), None)
        .await
        .unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(1,42)").unwrap(),
        &[],
    )
    .await
    .unwrap();
    assert_eq!(tx.prepare().await.unwrap().identity(), &identity);
    start.send(()).unwrap();
    finished.await.unwrap();
    assert!(fixture::business(&mut admin).await.is_empty());
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    fixture::record(
        "early-callback",
        serde_json::json!({"branch_id":identity.branch_id().to_string(),"xid":identity.xid(),"callback_statuses":[9,8],"early_result":"Unknown","original_prepared_rollback":"Applied","journal":"Completed","business_rows":0}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}
