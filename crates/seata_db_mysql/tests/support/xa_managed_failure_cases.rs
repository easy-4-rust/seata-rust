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
#[ignore = "requires owned MySQL and formal orphan callback checks"]
async fn early_callback_never_treats_orphan_evidence_as_empty_rollback() {
    let mut cases = Vec::new();
    for (index, table) in [
        "seata_rs_xa_commit_witness_v1",
        "seata_rs_xa_prepare_evidence_v1",
        "seata_rs_xa_guard_enrollment_v1",
        "seata_rs_xa_session_guard_v1",
        "native_prepared",
    ]
    .into_iter()
    .enumerate()
    {
        let (mut admin, source, adapter) = fixture::seed().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (manager, store) = rm(adapter.clone(), &listener, "early-orphan").await;
        let key = adapter.key().clone();
        let xid = format!("xa:early-orphan:{index}");
        let identity = BranchIdentity::new(
            xid.clone(),
            9007199254741020 + index as i64,
            key.clone(),
            None,
        )
        .unwrap();
        let native = seata_db_mysql::MySqlXaDataSource::new(source.clone());
        let native_id = seata_xa::XaBranchXid::new(Some(xid.clone()), identity.branch_id());
        if table == "native_prepared" {
            let mut scope = native.start(native_id.clone()).await.unwrap();
            scope
                .execute(
                    &SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(1,42)")
                        .unwrap(),
                    &[],
                )
                .await
                .unwrap();
            scope.prepare().await.unwrap();
        } else {
            let uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            let (phase, completed) = if table == "seata_rs_xa_commit_witness_v1" {
                (1, 1)
            } else {
                (0, 0)
            };
            sqlx::query(&format!("INSERT INTO {table}(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,NULL,?,1,?,?)"))
                .bind(xid.as_bytes()).bind(identity.branch_id()).bind(key.resource_id().as_bytes()).bind(uuid.as_bytes()).bind(phase).bind(completed).execute(&mut admin).await.unwrap();
        }
        let original = identity.clone();
        let (start, ready) = tokio::sync::oneshot::channel();
        let (first, observed) = tokio::sync::oneshot::channel();
        let (resume, repaired) = tokio::sync::oneshot::channel();
        let (done, finished) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = bootstrap(&listener, key.resource_id()).await;
            registered(
                &mut socket,
                key.resource_id(),
                &xid,
                None,
                original.branch_id(),
            )
            .await;
            ready.await.unwrap();
            let response = callback(&mut socket, -71, &original, BranchPhase::Rollback).await;
            assert_eq!(
                response,
                if table == "native_prepared" {
                    (ResultCode::Success, 9)
                } else {
                    (ResultCode::Failed, 10)
                },
                "orphan evidence was declared empty rollback"
            );
            first.send(()).unwrap();
            repaired.await.unwrap();
            assert_eq!(
                callback(&mut socket, -72, &original, BranchPhase::Rollback).await,
                (ResultCode::Success, 8)
            );
            done.send(()).unwrap();
            idle(&mut socket).await;
        });
        let receipt = manager
            .client()
            .branch_register_with_receipt(seata_protocol::transaction::BranchRegisterRequest {
                xid: Some(identity.xid().into()),
                branch_type: BranchType::Xa,
                resource_id: Some(identity.resource().resource_id().into()),
                application_data: None,
                lock_key: None,
            })
            .await
            .unwrap();
        start.send(()).unwrap();
        observed.await.unwrap();
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_xa_branch_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(
            rows, 0,
            "failed early proof committed a provisional denial record"
        );
        assert_eq!(
            store
                .lease(identity.clone(), BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Pending)
        );
        if table == "native_prepared" {
            native.rollback_prepared(&native_id).await.unwrap();
        } else {
            sqlx::raw_sql(&format!("DELETE FROM {table}"))
                .execute(&mut admin)
                .await
                .unwrap();
        }
        resume.send(()).unwrap();
        finished.await.unwrap();
        let error = adapter.start_registered(receipt).await.unwrap_err();
        assert_eq!(error.registered_branch().unwrap().identity(), &identity);
        assert!(fixture::business(&mut admin).await.is_empty());
        assert_eq!(
            store
                .lease(identity.clone(), BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        cases.push(serde_json::json!({"kind":table,"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"first_status":if table=="native_prepared" {9} else {10},"provisional_denial_rows_after_failure":rows,"first_journal":"Pending","repaired_status":8,"late_start_rejected":true,"journal":"Completed","business":[]}));
        manager.shutdown().await.unwrap();
        server.await.unwrap();
        source.close().await.unwrap();
    }
    fixture::record("early-orphan-evidence", serde_json::json!({"cases":cases}));
}

#[tokio::test]
#[ignore = "requires owned MySQL and real registration/callback race"]
async fn rollback_callback_races_original_start_without_late_business() {
    let mut cases = Vec::new();
    for index in 0..3 {
        let (mut admin, source, adapter) = fixture::seed().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (manager, store) = rm(adapter.clone(), &listener, "register-race").await;
        let key = adapter.key().clone();
        let xid = format!("xa:register-race:{index}");
        let identity = BranchIdentity::new(
            xid.clone(),
            9007199254741010 + index,
            key.clone(),
            Some("original-race-application".into()),
        )
        .unwrap();
        let original = identity.clone();
        let (done, finished) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = bootstrap(&listener, key.resource_id()).await;
            registered(
                &mut socket,
                key.resource_id(),
                &xid,
                Some("original-race-application"),
                original.branch_id(),
            )
            .await;
            let mut statuses = Vec::new();
            for retry in 0..4 {
                let response =
                    callback(&mut socket, -61 - retry, &original, BranchPhase::Rollback).await;
                statuses.push(response.1);
                if response == (ResultCode::Success, 8) {
                    break;
                }
                assert_eq!(
                    response.1, 9,
                    "registration race became an unretryable result"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            assert_eq!(
                statuses.last(),
                Some(&8),
                "registration/callback race did not finish original rollback"
            );
            done.send(statuses).unwrap();
            idle(&mut socket).await;
        });
        // 原登记响应与原回调相邻发送；生产者和控制事务实际竞争相同原键。
        let outcome = adapter
            .register_and_start(
                &manager.client(),
                identity.xid(),
                Some("original-race-application".into()),
            )
            .await;
        let statuses = finished.await.unwrap();
        let started = match outcome {
            Ok(mut tx) => {
                let error = tx
                    .execute(
                        &SqlRecognizerFactory::recognize(
                            "INSERT INTO xa_managed(id,v) VALUES(1,42)",
                        )
                        .unwrap(),
                        &[],
                    )
                    .await
                    .unwrap_err();
                assert_eq!(error.registered_branch().unwrap().identity(), &identity);
                assert!(tx.prepare().await.is_err());
                true
            }
            Err(error) => {
                assert_eq!(error.registered_branch().unwrap().identity(), &identity);
                false
            }
        };
        assert_eq!(
            store
                .lease(identity.clone(), BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        assert!(fixture::business(&mut admin).await.is_empty());
        cases.push(serde_json::json!({"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"application_data":identity.application_data(),"callback_statuses":statuses,"scope_started":started,"late_business_rejected":true,"journal":"Completed","business":[]}));
        manager.shutdown().await.unwrap();
        server.await.unwrap();
        source.close().await.unwrap();
    }
    fixture::record(
        "registration-callback-race",
        serde_json::json!({"cases":cases,"original_registration_each_case":1,"whole_plan_complete":false}),
    );
}

#[tokio::test]
#[ignore = "requires owned MySQL and live original ACTIVE scope"]
async fn original_active_scope_is_fenced_by_independent_rm_without_caller_drop() {
    fn send<F: Send>(future: F) -> F {
        future
    }
    let (mut admin, source, producer) = fixture::seed().await;
    let fresh_source = MySqlDataSource::connect(fixture::owned_mysql::options(None))
        .await
        .unwrap();
    let fresh = Arc::new(MySqlXaResourceManager::new(fresh_source.clone()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, store) = rm(fresh.clone(), &listener, "active-fence").await;
    let key = fresh.key().clone();
    let identity = BranchIdentity::new(
        "xa:active-fence".into(),
        9007199254741004,
        key.clone(),
        None,
    )
    .unwrap();
    let original = identity.clone();
    let (start, ready) = tokio::sync::oneshot::channel();
    let (done, finished) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = bootstrap(&listener, key.resource_id()).await;
        registered(
            &mut socket,
            key.resource_id(),
            "xa:active-fence",
            None,
            9007199254741004,
        )
        .await;
        ready.await.unwrap();
        assert_eq!(
            callback(&mut socket, -51, &original, BranchPhase::Rollback).await,
            (ResultCode::Success, 8),
            "live original ACTIVE scope was not fenced"
        );
        done.send(()).unwrap();
        idle(&mut socket).await;
    });
    let client = manager.client();
    let mut tx = send(producer.register_and_start(&client, identity.xid(), None))
        .await
        .unwrap();
    let sql = SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(1,42)").unwrap();
    send(tx.execute(&sql, &[])).await.unwrap();
    assert!(fixture::business(&mut admin).await.is_empty());
    // 先实际观察两次 phase=0 监视查询，再确认健康旧作用域仍能执行业务。
    let observations = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Execute' AND argument LIKE 'SELECT phase,completed FROM seata_rs_xa_branch_v1 WHERE xid=%'").fetch_one(&mut admin).await.unwrap();
            if count >= 2 { break count; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    send(tx.execute(
        &SqlRecognizerFactory::recognize("UPDATE xa_managed SET v=43 WHERE id=1").unwrap(),
        &[],
    ))
    .await
    .unwrap();
    start.send(()).unwrap();
    finished.await.unwrap();
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    let error = tx.execute(&sql, &[]).await.unwrap_err();
    assert_eq!(error.registered_branch().unwrap().identity(), &identity);
    assert!(send(tx.prepare()).await.is_err());
    assert!(fixture::business(&mut admin).await.is_empty());
    let guard: (u8, u8) =
        sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_session_guard_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!(guard, (2, 1));
    fixture::record(
        "active-scope-fence",
        serde_json::json!({"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"journal":"Completed","callback_status":8,"business":[],"guard":[2,1],"phase_zero_monitor_observations":observations,"caller_did_not_drop_before_callback":true,"independent_manager":true,"late_business_rejected":true,"late_prepare_rejected":true}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    fresh_source.close().await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned MySQL and original XA ROLLBACK OK loss"]
async fn lost_native_rollback_ok_recovers_original_prepared_intent_without_replay() {
    let (mut admin, plain, _) = fixture::seed().await;
    let original = seata_xa::XaBranchXid::new(Some("xa:rollback-loss".into()), 9007199254741003);
    let mut command = vec![3];
    command.extend_from_slice(
        format!("XA ROLLBACK {}", original.mysql_arguments().unwrap()).as_bytes(),
    );
    let proxy = xa_command_proxy::XaCommandProxy::start(
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        command.clone(),
    )
    .await;
    let mut prepare_command = vec![3];
    prepare_command.extend_from_slice(
        format!("XA PREPARE {}", original.mysql_arguments().unwrap()).as_bytes(),
    );
    let prepare_proxy =
        xa_command_proxy::XaCommandProxy::start(proxy.port, prepare_command.clone()).await;
    let source = MySqlDataSource::connect(
        fixture::owned_mysql::options(Some(prepare_proxy.port))
            .operation_timeout(Duration::from_millis(750))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = Arc::new(MySqlXaResourceManager::new(source.clone()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, store) = rm(adapter.clone(), &listener, "rollback-loss").await;
    let key = adapter.key().clone();
    let identity = BranchIdentity::new(
        "xa:rollback-loss".into(),
        9007199254741003,
        key.clone(),
        None,
    )
    .unwrap();
    let original_identity = identity.clone();
    let (start, ready) = tokio::sync::oneshot::channel();
    let (first, observed) = tokio::sync::oneshot::channel();
    let (resume, proceed) = tokio::sync::oneshot::channel();
    let (done, finished) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = bootstrap(&listener, key.resource_id()).await;
        registered(
            &mut socket,
            key.resource_id(),
            "xa:rollback-loss",
            None,
            9007199254741003,
        )
        .await;
        ready.await.unwrap();
        assert_eq!(
            callback(&mut socket, -41, &original_identity, BranchPhase::Rollback)
                .await
                .1,
            9
        );
        first.send(()).unwrap();
        proceed.await.unwrap();
        assert_eq!(
            callback(&mut socket, -42, &original_identity, BranchPhase::Rollback).await,
            (ResultCode::Success, 8),
            "original Prepared rollback intent did not resolve Pending"
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
    prepare_proxy.arm();
    let uncertain_prepare = tx.prepare().await.unwrap_err();
    assert_eq!(
        uncertain_prepare.registered_branch().unwrap().identity(),
        &identity
    );
    let (prepare_request, prepare_ok) = prepare_proxy.captured().unwrap();
    assert_eq!(&prepare_request[4..], prepare_command);
    assert_eq!(prepare_ok[4], 0);

    proxy.arm();
    start.send(()).unwrap();
    observed.await.unwrap();
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    // 故障代理原操作保留 750ms；独立常规管理器校验存储负例，不把调度抖动混入证据语义。
    let fresh_source =
        MySqlDataSource::connect(fixture::owned_mysql::options(Some(prepare_proxy.port)))
            .await
            .unwrap();
    let fresh = MySqlXaResourceManager::new(fresh_source.clone()).unwrap();
    // 此组验证旧 Prepared 意图路径，移除新 guard 协议，避免独立有效的 guard 证明替代该路径。
    sqlx::raw_sql("DELETE FROM seata_rs_xa_guard_enrollment_v1")
        .execute(&mut admin)
        .await
        .unwrap();
    let uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    let intent: (u8, u8) =
        sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_prepare_evidence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!(
        intent,
        (2, 0),
        "native rollback sent without persistent Prepared intent"
    );
    for assignment in [
        "resource_id='wrong'",
        "application_data='wrong'",
        "server_uuid='wrong'",
        "schema_version=9",
        "phase=1",
        "completed=1",
    ] {
        sqlx::raw_sql(&format!(
            "UPDATE seata_rs_xa_prepare_evidence_v1 SET {assignment}"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        assert!(
            matches!(
                fresh.recover(&identity, BranchPhase::Rollback).await,
                Err(seata_rm::RmError::IdentityConflict)
            ),
            "corrupt Prepared rollback intent accepted"
        );
        sqlx::query("UPDATE seata_rs_xa_prepare_evidence_v1 SET resource_id=?,application_data=NULL,server_uuid=?,schema_version=1,phase=2,completed=0").bind(identity.resource().resource_id().as_bytes()).bind(uuid.as_bytes()).execute(&mut admin).await.unwrap();
    }
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_prepare_evidence_v1 ADD KEY unexpected(resource_id)")
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(
        matches!(
            fresh.recover(&identity, BranchPhase::Rollback).await,
            Err(seata_rm::RmError::IdentityConflict)
        ),
        "unexpected intent index accepted"
    );
    sqlx::raw_sql("ALTER TABLE seata_rs_xa_prepare_evidence_v1 DROP KEY unexpected")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql("UPDATE seata_rs_xa_prepare_evidence_v1 SET phase=0")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        fresh
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown,
        "staged-only intent falsely confirmed rollback"
    );
    sqlx::raw_sql("DELETE FROM seata_rs_xa_prepare_evidence_v1")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        fresh
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown,
        "missing intent falsely confirmed rollback"
    );
    let completed: u8 = sqlx::query_scalar("SELECT completed FROM seata_rs_xa_branch_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(completed, 0);
    // 管理员恢复之前移除的同一原证据，只用于负例后的现场恢复。
    sqlx::raw_sql("INSERT INTO seata_rs_xa_prepare_evidence_v1 SELECT xid,branch_id,resource_id,application_data,server_uuid,schema_version,2,0 FROM seata_rs_xa_branch_v1").execute(&mut admin).await.unwrap();
    sqlx::raw_sql("INSERT INTO seata_rs_xa_commit_witness_v1 SELECT xid,branch_id,resource_id,application_data,server_uuid,schema_version,1,1 FROM seata_rs_xa_branch_v1").execute(&mut admin).await.unwrap();
    assert!(
        matches!(
            fresh.recover(&identity, BranchPhase::Rollback).await,
            Err(seata_rm::RmError::IdentityConflict)
        ),
        "visible Commit witness was accepted as rollback"
    );
    sqlx::raw_sql("DELETE FROM seata_rs_xa_commit_witness_v1")
        .execute(&mut admin)
        .await
        .unwrap();

    assert_eq!(
        fresh
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied,
        "fresh manager failed original Prepared rollback recovery"
    );
    fresh_source.close().await.unwrap();
    resume.send(()).unwrap();
    finished.await.unwrap();
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    assert!(fixture::business(&mut admin).await.is_empty());
    let witness: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_xa_commit_witness_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(witness, 0);
    let (request, response) = proxy.captured().unwrap();
    assert_eq!(&request[4..], command);
    assert_eq!(response[4], 0);
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    let prepare_count = prepare_proxy.shutdown().await;
    assert_eq!(
        prepare_count, 1,
        "original PREPARE was replayed after lost OK"
    );
    let count = proxy.shutdown().await;
    assert_eq!(count, 1);
    fixture::record(
        "native-rollback-loss",
        serde_json::json!({"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"request":request,"held_original_ok":response,"actual_commands":count,"prepare_request":prepare_request,"held_prepare_ok":prepare_ok,"actual_prepare_commands":prepare_count,"prepare_ack_lost":true,"callback_statuses":[9,8],"intermediate_journal":"Pending","journal":"Completed","business":[],"commit_witness_rows":0,"guard_protocol_removed_for_legacy_checks":true,"intent_corruptions_rejected":6,"intent_extra_index_rejected":true,"staged_only":"Unknown","missing_intent":"Unknown","commit_witness_conflict_rejected":true,"fresh_manager_recovered":true}),
    );
    source.close().await.unwrap();
    plain.close().await.unwrap();
}

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
async fn callback_before_registration_reply_fences_late_original_start() {
    let (mut admin, source, adapter) = fixture::seed().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (manager, store) = rm(adapter.clone(), &listener, "early").await;
    let key = adapter.key().clone();
    let identity =
        BranchIdentity::new("xa:early".into(), 9007199254741002, key.clone(), None).unwrap();
    assert_eq!(
        adapter
            .apply(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    assert_eq!(
        adapter
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    let manual_catalog = seata_rm::ResourceCatalog::new(4, 32767).unwrap();
    manual_catalog.register(adapter.clone()).unwrap();
    let manual = seata_rm::BranchDispatcher::new(manual_catalog, store.clone());
    assert_eq!(
        manual
            .dispatch(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap(),
        seata_protocol::BranchStatus::PhaseTwoRollbackFailedRetryable
    );
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_xa_branch_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(before, 0, "manual identity minted early rollback authority");
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
            (ResultCode::Success, 8),
            "early rollback did not persist fence before registration reply"
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
    let error = adapter
        .register_and_start(&manager.client(), identity.xid(), None)
        .await
        .unwrap_err();
    let receipt = error.registered_branch().unwrap().clone();
    assert_eq!(receipt.identity(), &identity);
    assert!(
        matches!(
            error,
            seata_db_mysql::MySqlXaEnlistError::Control {
                source: seata_db_mysql::DbError::Driver {
                    code: Some(1062),
                    ..
                },
                ..
            }
        ),
        "late original registration was not durably denied"
    );
    assert!(
        adapter.start_registered(receipt).await.is_err(),
        "late receipt clone restarted fenced branch"
    );
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
    let enrollment: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_xa_guard_enrollment_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!(
        enrollment, 0,
        "early callback fabricated producer enrollment"
    );
    let fresh_source = MySqlDataSource::connect(fixture::owned_mysql::options(None))
        .await
        .unwrap();
    let fresh = MySqlXaResourceManager::new(fresh_source.clone()).unwrap();
    assert_eq!(
        fresh
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert!(matches!(
        fresh.apply(&identity, BranchPhase::Commit).await,
        Err(seata_rm::RmError::IdentityConflict)
    ));
    fresh_source.close().await.unwrap();
    fixture::record(
        "early-callback",
        serde_json::json!({"branch_id":identity.branch_id().to_string(),"xid":identity.xid(),"callback_statuses":[8,8],"manual_entry_result":"Unknown","manual_dispatch_no_control_rows":true,"producer_enrollment_rows":0,"fresh_manager_terminal":"Applied","opposite_phase_rejected":true,"early_result":"Applied","late_start_rejected":true,"late_receipt_preserved":true,"duplicate_receipt_rejected":true,"journal":"Completed","business_rows":0}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}
