//! 报告清单已提交但 Try 未提交时，错误静态配置不得重放或污染原身份。
#![allow(clippy::unwrap_used)]
use super::{dynamic_action, fixture, manager, peer};
use seata_db_mysql::{MySqlDataSource, MySqlTccResourceManager};
use seata_protocol::ResultCode;
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResource, RegisteredBranch};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

async fn manifest_only(
    name: &str,
) -> (
    sqlx::MySqlConnection,
    MySqlDataSource,
    Arc<MySqlTccResourceManager>,
    RegisteredBranch,
    BranchIdentity,
) {
    let (mut admin, source, _) = fixture::seed().await;
    sqlx::raw_sql("DROP TABLE IF EXISTS seata_rs_tcc_context_v1")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/020_seata_rs_tcc_context_v1.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let dynamic = Arc::new(
        MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            Arc::new(dynamic_action::TccDynamicAction {
                immediate: false,
                precision_only: false,
                catch_report_error: false,
                failure_mode: None,
            }),
        )
        .unwrap()
        .with_context_reporting(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, store) = manager(dynamic.clone(), &listener, name).await;
    let xid = format!("tcc:configuration:{name}");
    let branch = 9901;
    let original = fixture::app(1);
    let expected = BranchIdentity::new(
        xid.clone(),
        branch,
        dynamic.key().clone(),
        Some(original.clone()),
    )
    .unwrap();
    let id = expected.clone();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, id.resource().resource_id()).await;
        peer::registered(
            &mut socket,
            id.resource().resource_id(),
            id.xid(),
            id.application_data(),
            branch,
        )
        .await;
        let (_, app) = peer::read_context_report(&mut socket, &id).await;
        drop(socket);
        app
    });
    let error = dynamic
        .register_and_try(&rm.client(), &xid, Some(original))
        .await
        .unwrap_err();
    let receipt = error.registered_branch().unwrap().clone();
    let seata_db_mysql::MySqlTccTryError::ContextReport {
        source: reason,
        context_report_started,
        ..
    } = error
    else {
        panic!("report uncertainty type lost")
    };
    assert!(context_report_started);
    let seata_db_mysql::MySqlTccContextReportError::Rpc(reason) = *reason else {
        panic!("expected original report RPC uncertainty")
    };
    assert!(reason.is_unknown_outcome());
    let app = server.await.unwrap();
    assert!(fixture::business(&mut admin).await.is_empty());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let versions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_context_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(versions, 2);
    assert_eq!(receipt.identity(), &expected);
    rm.shutdown().await.unwrap();
    drop(store);
    let current = BranchIdentity::new(xid, branch, dynamic.key().clone(), Some(app)).unwrap();
    (admin, source, dynamic, receipt, current)
}

#[tokio::test]
#[ignore = "requires owned MySQL and single-connection static compatibility"]
async fn static_pool_one_optional_table_and_existing_static_fence_remain_compatible() {
    let mut proofs = Vec::new();
    for (index, mode) in [
        "no_optional_table",
        "other_dynamic_branch",
        "static_origin0",
    ]
    .into_iter()
    .enumerate()
    {
        if mode == "other_dynamic_branch" {
            let (_, old, _, _, _) = manifest_only("unrelated-origin").await;
            old.close().await.unwrap();
        }
        let (mut admin, old, _) = fixture::seed().await;
        old.close().await.unwrap();
        if mode != "other_dynamic_branch" {
            sqlx::raw_sql("DROP TABLE IF EXISTS seata_rs_tcc_context_v1")
                .execute(&mut admin)
                .await
                .unwrap();
        }
        if mode == "static_origin0" {
            sqlx::raw_sql(include_str!(
                "../../migrations/020_seata_rs_tcc_context_v1.sql"
            ))
            .execute(&mut admin)
            .await
            .unwrap();
        }
        let source = MySqlDataSource::connect(
            fixture::owned_mysql::options(None)
                .pool_limits(1, Duration::from_secs(2))
                .unwrap(),
        )
        .await
        .unwrap();
        let adapter = Arc::new(
            MySqlTccResourceManager::new(
                source.clone(),
                "tcc-original-action".into(),
                Arc::new(fixture::Action::default()),
            )
            .unwrap(),
        );
        let id = BranchIdentity::new(
            format!("tcc:static-compat:{mode}"),
            10001 + index as i64,
            adapter.key().clone(),
            Some(fixture::app(1)),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, store) = manager(adapter.clone(), &listener, mode).await;
        let original = id.clone();
        let (send, ready) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = peer::bootstrap(&listener, original.resource().resource_id()).await;
            peer::registered(
                &mut socket,
                original.resource().resource_id(),
                original.xid(),
                original.application_data(),
                original.branch_id(),
            )
            .await;
            ready.await.unwrap();
            for request in [-510, -511] {
                assert_eq!(
                    peer::callback(&mut socket, request, &original, BranchPhase::Commit).await,
                    (ResultCode::Success, 5)
                );
            }
        });
        let receipt = adapter
            .register_and_try(
                &rm.client(),
                id.xid(),
                id.application_data().map(str::to_owned),
            )
            .await
            .unwrap();
        if mode == "static_origin0" {
            let report_source = MySqlDataSource::connect(fixture::owned_mysql::options(None))
                .await
                .unwrap();
            let dynamic = MySqlTccResourceManager::new(
                report_source.clone(),
                "tcc-original-action".into(),
                Arc::new(fixture::Action::default()),
            )
            .unwrap()
            .with_context_reporting();
            assert!(dynamic
                .try_registered_with_context(&rm.client(), receipt.clone())
                .await
                .is_err());
            let revisions: Vec<u16> =
                sqlx::query_scalar("SELECT revision FROM seata_rs_tcc_context_v1")
                    .fetch_all(&mut admin)
                    .await
                    .unwrap();
            assert_eq!(revisions, vec![0]);
            report_source.close().await.unwrap();
        }
        send.send(()).unwrap();
        server.await.unwrap();
        assert_eq!(
            store
                .lease(id.clone(), BranchPhase::Commit)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        let row: (u8, u16, Vec<u8>) = sqlx::query_as(
            "SELECT status,schema_version,application_data FROM seata_rs_tcc_fence_v1",
        )
        .fetch_one(&mut admin)
        .await
        .unwrap();
        assert_eq!((row.0, row.1), (2, 1));
        assert_eq!(row.2, id.application_data().unwrap().as_bytes());
        assert_eq!(fixture::business(&mut admin).await, vec![(1, 14)]);
        proofs.push(serde_json::json!({"mode":mode,"xid":id.xid(),"branch_id":id.branch_id().to_string(),"status":2,"protocol":1,"journal":"Completed","max_connections":1,"original_application_data":id.application_data(),"business":[[1,14]]}));
        rm.shutdown().await.unwrap();
        source.close().await.unwrap();
    }
    fixture::record(
        "configuration-static-compat",
        serde_json::json!({"cases":proofs}),
    );
}

#[tokio::test]
#[ignore = "requires owned MySQL users and actual optional-table access denial"]
async fn static_origin_access_denial_is_not_missing_table_and_correct_mode_recovers() {
    let name = "origin-permissions";
    let (mut admin, root, _, receipt, current) = manifest_only(name).await;
    sqlx::raw_sql("DROP USER IF EXISTS 'seata_tcc_mode_denied'@'%'; CREATE USER 'seata_tcc_mode_denied'@'%' IDENTIFIED BY 'fixture-tcc-mode-denied'").execute(&mut admin).await.unwrap();
    let database = std::env::var("SEATA_MYSQL_DATABASE").unwrap();
    assert!(database
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    for table in ["seata_rs_tcc_fence_v1", "tcc_managed"] {
        sqlx::raw_sql(&format!(
            "GRANT SELECT,INSERT,UPDATE,DELETE ON {}.{table} TO 'seata_tcc_mode_denied'@'%'",
            database
        ))
        .execute(&mut admin)
        .await
        .unwrap();
    }
    sqlx::raw_sql(&format!(
        "GRANT TRIGGER ON {}.* TO 'seata_tcc_mode_denied'@'%'",
        database
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::raw_sql("GRANT PROCESS ON *.* TO 'seata_tcc_mode_denied'@'%'")
        .execute(&mut admin)
        .await
        .unwrap();
    let limited = MySqlDataSource::connect(
        fixture::owned_mysql::options_for_user(
            None,
            "seata_tcc_mode_denied".into(),
            "fixture-tcc-mode-denied".into(),
        )
        .pool_limits(1, Duration::from_secs(2))
        .unwrap(),
    )
    .await
    .unwrap();
    let plain = Arc::new(
        MySqlTccResourceManager::new(
            limited.clone(),
            "tcc-original-action".into(),
            Arc::new(fixture::Action::default()),
        )
        .unwrap(),
    );
    let error = plain.try_registered(receipt.clone()).await.unwrap_err();
    assert!(
        matches!(
            error,
            seata_db_mysql::MySqlTccTryError::Refused {
                source: seata_db_mysql::DbError::Driver {
                    code: Some(1142),
                    ..
                },
                ..
            }
        ),
        "access denied must not become optional absence: {error:?}"
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, store) = manager(plain, &listener, name).await;
    let id = current.clone();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, id.resource().resource_id()).await;
        peer::callback(&mut socket, -520, &id, BranchPhase::Rollback).await
    });
    assert_eq!(server.await.unwrap(), (ResultCode::Failed, 9));
    assert!(store
        .lease(current.clone(), BranchPhase::Rollback)
        .await
        .unwrap()
        .state()
        .is_none());
    assert!(fixture::business(&mut admin).await.is_empty());
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    rm.shutdown().await.unwrap();
    drop(store);
    sqlx::raw_sql(&format!(
        "GRANT SELECT,INSERT,UPDATE,DELETE ON {}.seata_rs_tcc_context_v1 TO 'seata_tcc_mode_denied'@'%'",
        database
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let dynamic = Arc::new(
        MySqlTccResourceManager::new(
            limited.clone(),
            "tcc-original-action".into(),
            Arc::new(fixture::Action::default()),
        )
        .unwrap()
        .with_context_reporting(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, reopened) = manager(dynamic, &listener, name).await;
    let id = current.clone();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, id.resource().resource_id()).await;
        peer::callback(&mut socket, -521, &id, BranchPhase::Rollback).await
    });
    assert_eq!(server.await.unwrap(), (ResultCode::Success, 8));
    assert_eq!(
        reopened
            .lease(receipt.identity().clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    let row: (u8, u16, Vec<u8>) =
        sqlx::query_as("SELECT status,schema_version,application_data FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!((row.0, row.1), (4, 2));
    assert_eq!(
        row.2,
        receipt.identity().application_data().unwrap().as_bytes()
    );
    fixture::record(
        "configuration-permission",
        serde_json::json!({"xid":current.xid(),"branch_id":current.branch_id().to_string(),"denied_code":1142,"before_grant_journal":"None","before_grant_fence_rows":0,"after_grant_correct_mode":"Completed","status":4,"protocol":2,"max_connections":1,"original_application_data":receipt.identity().application_data(),"reported_application_data":current.application_data(),"business":[]}),
    );
    rm.shutdown().await.unwrap();
    limited.close().await.unwrap();
    root.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned MySQL and durable dynamic report origin"]
async fn static_try_cannot_replay_original_receipt_after_unconfirmed_dynamic_report() {
    let (mut admin, source, _, receipt, current) = manifest_only("static-try").await;
    let plain = MySqlTccResourceManager::new(
        source.clone(),
        "tcc-original-action".into(),
        Arc::new(fixture::Action::default()),
    )
    .unwrap();
    let result = plain.try_registered(receipt.clone()).await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    let business = fixture::business(&mut admin).await;
    fixture::record(
        "configuration-static-try",
        serde_json::json!({"xid":current.xid(),"branch_id":current.branch_id().to_string(),"accepted":result.is_ok(),"fence_rows":count,"business":business,"original_receipt_retained":result.as_ref().err().and_then(|e|e.registered_branch()).is_some_and(|r|r.identity()==receipt.identity())}),
    );
    assert!(
        result.is_err(),
        "static Try must reject a branch with durable dynamic origin"
    );
    assert_eq!(
        result.unwrap_err().registered_branch().unwrap().identity(),
        receipt.identity()
    );
    assert_eq!(count, 0);
    assert!(business.is_empty());
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned MySQL and configuration transition recovery"]
async fn static_callback_cannot_poison_manifest_only_origin_before_correct_configuration_recovers()
{
    let name = "static-callback";
    let (mut admin, source, dynamic, receipt, current) = manifest_only(name).await;
    let plain = Arc::new(
        MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            Arc::new(fixture::Action::default()),
        )
        .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, store) = manager(plain, &listener, name).await;
    let id = current.clone();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, id.resource().resource_id()).await;
        peer::callback(&mut socket, -490, &id, BranchPhase::Rollback).await
    });
    let response = server.await.unwrap();
    let state = store
        .lease(current.clone(), BranchPhase::Rollback)
        .await
        .unwrap()
        .state();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    fixture::record(
        "configuration-static-callback",
        serde_json::json!({"stage":"static_attempt","xid":current.xid(),"branch_id":current.branch_id().to_string(),"result":format!("{:?}",response.0),"branch_status":response.1,"fence_rows":count,"journal":format!("{state:?}")}),
    );
    assert_eq!(
        response,
        (ResultCode::Failed, 9),
        "static callback must reject dynamic origin before journal or Fence writes"
    );
    assert!(state.is_none());
    assert_eq!(count, 0);
    rm.shutdown().await.unwrap();
    drop(store);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, reopened) = manager(dynamic.clone(), &listener, name).await;
    let id = current.clone();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, id.resource().resource_id()).await;
        peer::callback(&mut socket, -491, &id, BranchPhase::Rollback).await
    });
    assert_eq!(server.await.unwrap(), (ResultCode::Success, 8));
    assert_eq!(
        reopened
            .lease(receipt.identity().clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    let row: (u8, u16, Vec<u8>) =
        sqlx::query_as("SELECT status,schema_version,application_data FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!((row.0, row.1), (4, 2));
    assert_eq!(
        row.2,
        receipt.identity().application_data().unwrap().as_bytes()
    );
    assert!(dynamic
        .try_registered_with_context(&rm.client(), receipt.clone())
        .await
        .is_err());
    assert!(fixture::business(&mut admin).await.is_empty());
    fixture::record(
        "configuration-static-callback",
        serde_json::json!({"stage":"completed","xid":current.xid(),"branch_id":current.branch_id().to_string(),"static_result":"Failed","static_branch_status":9,"static_fence_rows":0,"static_journal":"None","journal":"Completed","status":4,"protocol":2,"original_application_data":receipt.identity().application_data(),"reported_application_data":current.application_data(),"late_try_rejected":true,"business":[]}),
    );
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
