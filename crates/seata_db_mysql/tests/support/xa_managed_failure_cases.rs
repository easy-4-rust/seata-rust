//! 真实成功响应丢失及早于登记响应的回调；不得把暂时缺失误报为终态。
#![allow(clippy::unwrap_used)]
#[path = "xa_command_proxy.rs"]
mod xa_command_proxy;
use super::{bootstrap, callback, fixture, idle, peer, registered, rm};
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::{MySqlDataSource, MySqlXaResourceManager};
use seata_protocol::{transaction::BranchRegisterResponse, BranchType, Message, ResultCode};
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResolution, BranchResource};
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
    let (first_done, first_finished) = tokio::sync::oneshot::channel();
    let (resume, proceed) = tokio::sync::oneshot::channel();
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
        first_done.send(()).unwrap();
        proceed.await.unwrap();
        assert_eq!(
            callback(&mut socket, -28, &peer_identity, BranchPhase::Commit).await,
            (ResultCode::Success, 5),
            "committed same-XA witness did not resolve original Pending"
        );
        done.send(()).unwrap();
        idle(&mut socket).await;
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
    let invisible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_xa_commit_witness_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        invisible, 0,
        "Prepared witness leaked before original COMMIT"
    );
    proxy.arm();
    start.send(()).unwrap();
    first_finished.await.unwrap();
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    let uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    for assignment in [
        "resource_id='wrong'",
        "application_data='wrong'",
        "server_uuid='wrong'",
        "schema_version=9",
        "phase=0",
        "completed=0",
    ] {
        sqlx::raw_sql(&format!(
            "UPDATE seata_rs_xa_commit_witness_v1 SET {assignment}"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        assert!(
            adapter
                .recover(&identity, BranchPhase::Commit)
                .await
                .is_err(),
            "corrupt XA witness accepted as original commit proof"
        );
        sqlx::query("UPDATE seata_rs_xa_commit_witness_v1 SET resource_id=?,application_data=NULL,server_uuid=?,schema_version=1,phase=1,completed=1")
            .bind(identity.resource().resource_id().as_bytes()).bind(uuid.as_bytes()).execute(&mut admin).await.unwrap();
    }
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_commit_witness_v1 ADD KEY unexpected(resource_id)")
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(
        adapter
            .recover(&identity, BranchPhase::Commit)
            .await
            .is_err(),
        "unexpected witness index accepted"
    );
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_commit_witness_v1 DROP KEY unexpected")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql("DELETE FROM seata_rs_xa_commit_witness_v1")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        adapter
            .recover(&identity, BranchPhase::Commit)
            .await
            .unwrap(),
        BranchResolution::Unknown,
        "missing witness falsely proved original COMMIT"
    );
    let incomplete: u8 = sqlx::query_scalar("SELECT completed FROM seata_rs_xa_branch_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        incomplete, 0,
        "rejected/missing witness changed original terminal"
    );
    // 独立管理员只恢复测试先前移走的原见证字段，不为生产恢复合成身份。
    sqlx::raw_sql("INSERT INTO seata_rs_xa_commit_witness_v1 SELECT xid,branch_id,resource_id,application_data,server_uuid,schema_version,1,1 FROM seata_rs_xa_branch_v1").execute(&mut admin).await.unwrap();
    let fresh_source = MySqlDataSource::connect(fixture::owned_mysql::options(Some(proxy.port)))
        .await
        .unwrap();
    let fresh = MySqlXaResourceManager::new(fresh_source.clone()).unwrap();
    assert_eq!(
        fresh.recover(&identity, BranchPhase::Commit).await.unwrap(),
        BranchResolution::Applied,
        "fresh resource failed original committed witness recovery"
    );
    fresh_source.close().await.unwrap();
    resume.send(()).unwrap();
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
        Some(BranchRecordState::Completed),
        "committed same-XA witness did not finish original RM journal"
    );
    let row: (u8, u8) = sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_branch_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(row, (1, 1));
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
        serde_json::json!({"branch_id":identity.branch_id().to_string(),"xid":identity.xid(),"request":request,"held_original_ok":response,"actual_commands":count,"callback_statuses":[6,5],"witness_invisible_before_commit":true,"witness_corruptions_rejected":6,"witness_extra_index_rejected":true,"missing_witness":"Unknown","fresh_manager_recovered":true,"intermediate_journal":"Pending","journal":"Completed","control_state":[1,1],"business":[[1,42]],"missing_prepared_not_success":true}),
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
        idle(&mut socket).await;
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
