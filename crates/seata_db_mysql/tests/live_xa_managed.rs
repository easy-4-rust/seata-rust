//! 正式 RM 客户端与真实 MySQL；TCP 对端只签发原登记/回调，真实资源动作由生产适配器执行。
#![cfg(feature = "xa")]
#![allow(clippy::unwrap_used)]
#[path = "support/xa_managed_failure_cases.rs"]
mod failure_cases;
#[path = "support/xa_managed_fixture.rs"]
mod fixture;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;
use rm_fixture::peer;
use seata_at::SqlRecognizerFactory;
use seata_datasource::FieldValue;
use seata_db_mysql::{DbError, MySqlXaEnlistError, MySqlXaResourceManager};
use seata_protocol::{
    protocol::RegisterRMResponse,
    transaction::{BranchCommitRequest, BranchRegisterResponse, BranchRollbackRequest},
    BranchType, Message, RequestType, ResultCode,
};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchResolution, BranchResource, FileBranchStore, JournalOptions,
    ResourceCatalog, ResourceManager,
};
use std::{sync::Arc, time::Duration};
use tokio::net::{TcpListener, TcpStream};

async fn rm(
    adapter: Arc<MySqlXaResourceManager>,
    listener: &TcpListener,
    name: &str,
) -> (ResourceManager, FileBranchStore) {
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter).unwrap();
    let store = FileBranchStore::open(
        fixture::folder(name).join("branches.log"),
        JournalOptions::new(32, 1048576, 65536, 32).unwrap(),
    )
    .await
    .unwrap();
    let manager = ResourceManager::builder(
        fixture::pool(listener.local_addr().unwrap()),
        fixture::registration(false),
        catalog,
        store.clone(),
    )
    .request_timeout(if matches!(name, "early" | "register-race") {
        Duration::from_secs(5)
    } else {
        Duration::from_millis(1200)
    })
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    (manager, store)
}
// 现场元数据篡改矩阵可超过通用短夹具的 EOF 等待；仍拒绝任何额外请求。
async fn idle(socket: &mut TcpStream) {
    use tokio::io::AsyncReadExt;
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(30), socket.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}
async fn bootstrap(listener: &TcpListener, resource: &str) -> TcpStream {
    let (mut socket, _) = listener.accept().await.unwrap();
    let request = peer::read(&mut socket).await;
    let Some(Message::RegisterRMRequest(fields)) = request.body else {
        panic!("missing original RM registration")
    };
    assert_eq!(fields.resource_ids.as_deref(), Some(resource));
    assert_eq!(fields.application_id.as_deref(), Some("rm-app"));
    peer::write(
        &mut socket,
        peer::reply(
            request.id,
            Message::RegisterRMResponse(RegisterRMResponse {
                identified: true,
                wire_version: None,
            }),
        ),
    )
    .await;
    socket
}
async fn registered(
    socket: &mut TcpStream,
    resource: &str,
    xid: &str,
    app: Option<&str>,
    branch: i64,
) {
    let request = peer::read(socket).await;
    let Some(Message::BranchRegisterRequest(fields)) = request.body else {
        panic!("XA branch registration missing")
    };
    assert_eq!(fields.branch_type, BranchType::Xa);
    assert_eq!(fields.resource_id.as_deref(), Some(resource));
    assert_eq!(fields.xid.as_deref(), Some(xid));
    assert_eq!(fields.application_data.as_deref(), app);
    assert_eq!(fields.lock_key, None);
    peer::write(
        socket,
        peer::reply(
            request.id,
            Message::BranchRegisterResponse(BranchRegisterResponse {
                response: peer::success(),
                branch_id: branch,
            }),
        ),
    )
    .await;
}
async fn callback(
    socket: &mut TcpStream,
    id: i32,
    identity: &BranchIdentity,
    phase: BranchPhase,
) -> (ResultCode, u8) {
    let body = match phase {
        BranchPhase::Commit => Message::BranchCommitRequest(BranchCommitRequest {
            xid: identity.xid().into(),
            branch_id: identity.branch_id(),
            branch_type: BranchType::Xa,
            resource_id: identity.resource().resource_id().into(),
            application_data: identity.application_data().map(str::to_owned),
        }),
        BranchPhase::Rollback => Message::BranchRollbackRequest(BranchRollbackRequest {
            xid: identity.xid().into(),
            branch_id: identity.branch_id(),
            branch_type: BranchType::Xa,
            resource_id: identity.resource().resource_id().into(),
            application_data: identity.application_data().map(str::to_owned),
        }),
    };
    let mut frame = peer::reply(id, body);
    frame.message_type = RequestType::RequestSync;
    peer::write(socket, frame).await;
    let response = peer::read(socket).await;
    assert_eq!(response.id, id);
    match response.body.unwrap() {
        Message::BranchCommitResponse(value) => {
            assert_eq!(value.branch_id, identity.branch_id());
            (value.response.result_code, value.branch_status.to_java_v1())
        }
        Message::BranchRollbackResponse(value) => {
            assert_eq!(value.branch_id, identity.branch_id());
            (value.response.result_code, value.branch_status.to_java_v1())
        }
        other => panic!("unexpected callback response: {other:?}"),
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL and managed XA schema"]
async fn registered_original_xa_prepares_and_completes_through_real_rm_callbacks() {
    let (mut admin, source, adapter) = fixture::seed().await;
    let mut proofs = Vec::new();
    for (index, phase) in [BranchPhase::Commit, BranchPhase::Rollback]
        .into_iter()
        .enumerate()
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (manager, store) = rm(adapter.clone(), &listener, &format!("normal-{index}")).await;
        let key = adapter.key().clone();
        let xid = format!("xa:managed:{index}");
        let branch = 9007199254740995 + index as i64;
        let app = if index == 0 {
            Some("xa-private-original-app")
        } else {
            None
        };
        let identity =
            BranchIdentity::new(xid.clone(), branch, key.clone(), app.map(str::to_owned)).unwrap();
        let original = identity.clone();
        let (start, ready) = tokio::sync::oneshot::channel();
        let (done, finished) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = bootstrap(&listener, key.resource_id()).await;
            registered(&mut socket, key.resource_id(), &xid, app, branch).await;
            ready.await.unwrap();
            let expected = if phase == BranchPhase::Commit { 5 } else { 8 };
            assert_eq!(
                callback(&mut socket, i32::MIN, &original, phase).await,
                (ResultCode::Success, expected)
            );
            assert_eq!(
                callback(&mut socket, -17, &original, phase).await,
                (ResultCode::Success, expected)
            );
            let opposite = if phase == BranchPhase::Commit {
                BranchPhase::Rollback
            } else {
                BranchPhase::Commit
            };
            assert_eq!(
                callback(&mut socket, -18, &original, opposite).await,
                (
                    ResultCode::Failed,
                    if phase == BranchPhase::Commit { 10 } else { 7 }
                )
            );
            done.send(()).unwrap();
            idle(&mut socket).await;
        });
        let mut enlisted = adapter
            .register_and_start(&manager.client(), identity.xid(), app.map(str::to_owned))
            .await
            .unwrap();
        let receipt = enlisted.registered_branch().clone();
        assert_eq!(receipt.identity(), &identity);
        let duplicate = adapter.start_registered(receipt.clone()).await.unwrap_err();
        assert!(
            matches!(
                &duplicate,
                MySqlXaEnlistError::Control {
                    source: DbError::Driver {
                        code: Some(1062),
                        ..
                    },
                    ..
                }
            ),
            "duplicate receipt was not rejected by durable pre-START claim"
        );
        assert_eq!(duplicate.registered_branch().unwrap().identity(), &identity);
        enlisted
            .execute(
                &SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(?,?)")
                    .unwrap(),
                &[
                    FieldValue::Signed(index as i64 + 1),
                    FieldValue::Signed(10 + index as i64),
                ],
            )
            .await
            .unwrap();
        let prepared = enlisted.prepare().await.unwrap();
        assert_eq!(prepared.identity(), &identity);
        assert!(fixture::business(&mut admin)
            .await
            .iter()
            .all(|(id, _)| *id != index as i32 + 1));
        start.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5),async{loop{let rows:i64=sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_xa_branch_v1 WHERE xid=? AND branch_id=? AND completed=1").bind(identity.xid().as_bytes()).bind(branch).fetch_one(&mut admin).await.unwrap();if rows==1{break;}tokio::time::sleep(Duration::from_millis(5)).await;}}).await.unwrap();
        // 等待两次同向和一次相反回调完成，随后读取原持久日志。
        finished.await.unwrap();
        let witness_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM seata_rs_xa_commit_witness_v1 WHERE xid=? AND branch_id=?",
        )
        .bind(identity.xid().as_bytes())
        .bind(branch)
        .fetch_one(&mut admin)
        .await
        .unwrap();
        assert_eq!(
            witness_count,
            if phase == BranchPhase::Commit { 1 } else { 0 },
            "witness was not atomic with original XA direction"
        );
        let state = store.lease(identity.clone(), phase).await.unwrap().state();
        assert_eq!(state, Some(seata_rm::BranchRecordState::Completed));
        let fresh_source =
            seata_db_mysql::MySqlDataSource::connect(fixture::owned_mysql::options(None))
                .await
                .unwrap();
        let fresh = MySqlXaResourceManager::new(fresh_source.clone()).unwrap();
        assert_eq!(
            fresh.recover(&identity, phase).await.unwrap(),
            BranchResolution::Applied,
            "fresh manager lost durable original terminal"
        );
        let wrong = if phase == BranchPhase::Commit {
            BranchPhase::Rollback
        } else {
            BranchPhase::Commit
        };
        assert!(
            fresh.recover(&identity, wrong).await.is_err(),
            "durable XA direction was changed by new manager"
        );
        fresh_source.close().await.unwrap();
        let value: Option<i32> = sqlx::query_scalar("SELECT v FROM xa_managed WHERE id=?")
            .bind(index as i32 + 1)
            .fetch_optional(&mut admin)
            .await
            .unwrap();
        assert_eq!(value, if index == 0 { Some(10) } else { None });
        proofs.push(serde_json::json!({"xid":identity.xid(),"branch_id":branch.to_string(),"resource_id":identity.resource().resource_id(),"app_present":app.is_some(),"phase":if index==0{"Commit"}else{"Rollback"},"business_value":value,"original_journal":"Completed","fresh_manager":"Applied","duplicate_start_rejected":true}));
        manager.shutdown().await.unwrap();
        server.await.unwrap();
    }
    fixture::record(
        "managed-normal",
        serde_json::json!({"branches":proofs,"phase_one_reports":0}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL"]
async fn failed_or_lost_registration_cannot_start_mysql_xa() {
    let (mut admin, source, adapter) = fixture::seed().await;
    for lost in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (manager, _) = rm(
            adapter.clone(),
            &listener,
            if lost { "lost" } else { "rejected" },
        )
        .await;
        let key = adapter.key().clone();
        let server = tokio::spawn(async move {
            let mut socket = bootstrap(&listener, key.resource_id()).await;
            let request = peer::read(&mut socket).await;
            assert!(
                matches!(request.body,Some(Message::BranchRegisterRequest(ref v)) if v.branch_type==BranchType::Xa&&v.lock_key.is_none())
            );
            if !lost {
                let mut failure = peer::success();
                failure.result_code = ResultCode::Failed;
                peer::write(
                    &mut socket,
                    peer::reply(
                        request.id,
                        Message::BranchRegisterResponse(BranchRegisterResponse {
                            response: failure,
                            branch_id: 0,
                        }),
                    ),
                )
                .await;
                idle(&mut socket).await;
            }
        });
        let oversized = adapter
            .register_and_start(&manager.client(), &"x".repeat(65), None)
            .await
            .unwrap_err();
        assert!(matches!(oversized, MySqlXaEnlistError::Local { .. }));
        assert!(oversized.registered_branch().is_none());
        let error = adapter
            .register_and_start(
                &manager.client(),
                if lost { "xa:lost" } else { "xa:rejected" },
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, MySqlXaEnlistError::Registration(_)));
        assert!(error.registered_branch().is_none());
        manager.shutdown().await.unwrap();
        server.await.unwrap();
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM seata_rs_xa_branch_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        0
    );
    assert!(sqlx::raw_sql("XA RECOVER")
        .fetch_all(&mut admin)
        .await
        .unwrap()
        .is_empty());
    assert!(fixture::business(&mut admin).await.is_empty());
    fixture::record(
        "registration-failures",
        serde_json::json!({"rejected_and_lost":2,"claim_rows":0,"business_rows":0,"prepared":0}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL"]
async fn unsafe_claim_table_rejects_before_xa_start_and_keeps_original_receipt() {
    let (mut admin, source, adapter) = fixture::seed().await;
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_branch_v1 ENGINE=MyISAM")
        .execute(&mut admin)
        .await
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, _) = rm(adapter.clone(), &listener, "unsafe").await;
    let key = adapter.key().clone();
    let server = tokio::spawn(async move {
        let mut socket = bootstrap(&listener, key.resource_id()).await;
        registered(
            &mut socket,
            key.resource_id(),
            "xa:unsafe",
            None,
            9007199254740997,
        )
        .await;
        idle(&mut socket).await;
    });
    let error = adapter
        .register_and_start(&manager.client(), "xa:unsafe", None)
        .await
        .unwrap_err();
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        9007199254740997
    );
    assert!(
        matches!(error, MySqlXaEnlistError::Control { .. }),
        "unsafe nontransactional XA claim did not reject before START"
    );
    assert!(sqlx::raw_sql("XA RECOVER")
        .fetch_all(&mut admin)
        .await
        .unwrap()
        .is_empty());
    fixture::record(
        "unsafe-claim",
        serde_json::json!({"registered_branch":"9007199254740997","claim_rejected":true,"native_start_forbidden":true}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL"]
async fn bound_direction_closes_active_scope_and_absence_stays_unknown() {
    let (mut admin, source, adapter) = fixture::seed().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, _) = rm(adapter.clone(), &listener, "bound").await;
    let key = adapter.key().clone();
    let server = tokio::spawn(async move {
        let mut socket = bootstrap(&listener, key.resource_id()).await;
        registered(
            &mut socket,
            key.resource_id(),
            "xa:bound",
            None,
            9007199254740998,
        )
        .await;
        idle(&mut socket).await;
    });
    let mut tx = adapter
        .register_and_start(&manager.client(), "xa:bound", None)
        .await
        .unwrap();
    let identity = tx.registered_branch().identity().clone();
    // 没有新协议标记的旧记录不能仅凭缺 Prepared 被补成成功。
    sqlx::raw_sql("DELETE FROM seata_rs_xa_guard_enrollment_v1")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        adapter
            .apply(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown,
        "missing Prepared was incorrectly called Applied"
    );
    let error = tx
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(1,10)").unwrap(),
            &[],
        )
        .await
        .unwrap_err();
    assert_eq!(error.registered_branch().unwrap().identity(), &identity);
    assert!(tx.prepare().await.is_err());
    assert!(fixture::business(&mut admin).await.is_empty());
    assert_eq!(
        adapter
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    assert!(adapter.apply(&identity, BranchPhase::Commit).await.is_err());
    let uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    // 恢复先前移除的原协议登记，再分别注入不可信控制记录验证拒绝。
    sqlx::raw_sql("INSERT INTO seata_rs_xa_guard_enrollment_v1 SELECT xid,branch_id,resource_id,application_data,server_uuid,schema_version,0,0 FROM seata_rs_xa_branch_v1").execute(&mut admin).await.unwrap();
    for assignment in [
        "resource_id='wrong'",
        "application_data='wrong'",
        "server_uuid='wrong'",
        "schema_version=9",
        "phase=1",
        "completed=1",
    ] {
        sqlx::raw_sql(&format!(
            "UPDATE seata_rs_xa_guard_enrollment_v1 SET {assignment}"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        assert!(
            matches!(
                adapter.recover(&identity, BranchPhase::Rollback).await,
                Err(seata_rm::RmError::IdentityConflict)
            ),
            "invalid guard enrollment granted rollback authority"
        );
        sqlx::query("UPDATE seata_rs_xa_guard_enrollment_v1 SET resource_id=?,application_data=NULL,server_uuid=?,schema_version=1,phase=0,completed=0").bind(identity.resource().resource_id().as_bytes()).bind(uuid.as_bytes()).execute(&mut admin).await.unwrap();
    }
    for assignment in [
        "resource_id='wrong'",
        "application_data='wrong'",
        "server_uuid='wrong'",
        "schema_version=9",
        "phase=0,completed=0",
        "completed=0",
    ] {
        sqlx::raw_sql("INSERT INTO seata_rs_xa_session_guard_v1 SELECT xid,branch_id,resource_id,application_data,server_uuid,schema_version,2,1 FROM seata_rs_xa_branch_v1").execute(&mut admin).await.unwrap();
        sqlx::raw_sql(&format!(
            "UPDATE seata_rs_xa_session_guard_v1 SET {assignment}"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        assert!(
            matches!(
                adapter.recover(&identity, BranchPhase::Rollback).await,
                Err(seata_rm::RmError::IdentityConflict)
            ),
            "invalid or committed guard accepted as rollback fence"
        );
        sqlx::raw_sql("DELETE FROM seata_rs_xa_session_guard_v1")
            .execute(&mut admin)
            .await
            .unwrap();
    }
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_session_guard_v1 ADD KEY unexpected(resource_id)")
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(
        matches!(
            adapter.recover(&identity, BranchPhase::Rollback).await,
            Err(seata_rm::RmError::IdentityConflict)
        ),
        "unexpected guard index accepted"
    );
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_session_guard_v1 DROP KEY unexpected")
        .execute(&mut admin)
        .await
        .unwrap();
    let incomplete: u8 = sqlx::query_scalar("SELECT completed FROM seata_rs_xa_branch_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(incomplete, 0);
    assert_eq!(
        adapter
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied,
        "valid original guard fence did not complete after repair"
    );
    fixture::record(
        "bound-and-unknown",
        serde_json::json!({"branch_id":identity.branch_id().to_string(),"rollback":"Unknown","missing_guard_enrollment":"Unknown","enrollment_corruptions_rejected":6,"guard_corruptions_rejected":6,"guard_index_rejected":true,"restored_guard_fence":"Applied","business_after_binding_rejected":true,"opposite_direction_rejected":true}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL"]
async fn persisted_original_identity_and_current_schema_guard_native_finish() {
    let (mut admin, source, adapter) = fixture::seed().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, _) = rm(adapter.clone(), &listener, "identity").await;
    let key = adapter.key().clone();
    let server = tokio::spawn(async move {
        let mut socket = bootstrap(&listener, key.resource_id()).await;
        registered(
            &mut socket,
            key.resource_id(),
            "xa:identity",
            None,
            9007199254740999,
        )
        .await;
        idle(&mut socket).await;
    });
    let mut tx = adapter
        .register_and_start(&manager.client(), "xa:identity", None)
        .await
        .unwrap();
    let receipt = tx.registered_branch().clone();
    let identity = receipt.identity().clone();
    tx.execute(
        &SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(1,42)").unwrap(),
        &[],
    )
    .await
    .unwrap();
    tx.prepare().await.unwrap();
    let uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    for assignment in [
        "resource_id='wrong'",
        "application_data='wrong'",
        "server_uuid='wrong'",
        "schema_version=9",
        "phase=2",
        "completed=1",
    ] {
        sqlx::raw_sql(&format!("UPDATE seata_rs_xa_branch_v1 SET {assignment}"))
            .execute(&mut admin)
            .await
            .unwrap();
        assert!(
            adapter.apply(&identity, BranchPhase::Commit).await.is_err(),
            "corrupt original XA identity allowed native finish"
        );
        sqlx::query("UPDATE seata_rs_xa_branch_v1 SET resource_id=?,application_data=NULL,server_uuid=?,schema_version=1,phase=0,completed=0").bind(adapter.key().resource_id().as_bytes()).bind(uuid.as_bytes()).execute(&mut admin).await.unwrap();
    }
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_branch_v1 ADD KEY unexpected(resource_id)")
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(
        adapter
            .recover(&identity, BranchPhase::Commit)
            .await
            .is_err(),
        "unexpected XA control index was accepted"
    );
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_branch_v1 DROP KEY unexpected")
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(fixture::business(&mut admin).await.is_empty());
    assert_eq!(
        adapter
            .recover(&identity, BranchPhase::Commit)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(fixture::business(&mut admin).await, vec![(1, 42)]);
    fixture::record(
        "identity-and-schema",
        serde_json::json!({"identity_corruptions_rejected":6,"extra_index_rejected":true,"restored_original_branch":"Applied","business":[[1,42]]}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL"]
async fn changed_commit_durability_after_connect_blocks_xa_start() {
    let (mut admin, source, adapter) = fixture::seed().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, _) = rm(adapter.clone(), &listener, "durability").await;
    let key = adapter.key().clone();
    let server = tokio::spawn(async move {
        let mut socket = bootstrap(&listener, key.resource_id()).await;
        registered(
            &mut socket,
            key.resource_id(),
            "xa:durability",
            None,
            9007199254741000,
        )
        .await;
        idle(&mut socket).await;
    });
    sqlx::raw_sql("SET GLOBAL innodb_flush_log_at_trx_commit=0")
        .execute(&mut admin)
        .await
        .unwrap();
    let result = adapter
        .register_and_start(&manager.client(), "xa:durability", None)
        .await;
    sqlx::raw_sql("SET GLOBAL innodb_flush_log_at_trx_commit=1")
        .execute(&mut admin)
        .await
        .unwrap();
    let error = result.unwrap_err();
    assert!(
        matches!(error, MySqlXaEnlistError::Control { .. }),
        "XA accepted changed non-durable control configuration"
    );
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        9007199254741000
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM seata_rs_xa_branch_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        0
    );
    assert!(fixture::business(&mut admin).await.is_empty());
    fixture::record(
        "durability",
        serde_json::json!({"registered_branch":"9007199254741000","non_durable_after_connect_rejected":true,"claim_rows":0,"native_start_forbidden":true,"configuration_restored":true}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}
