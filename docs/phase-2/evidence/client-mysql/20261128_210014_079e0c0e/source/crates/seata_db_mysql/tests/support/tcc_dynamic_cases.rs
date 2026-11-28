//! 原报告版本、旧 Pending 和合法新上下文回调的真实数据库验收。
#![allow(clippy::unwrap_used)]
use super::dynamic_action as action;
use super::{fixture, manager, peer};
use seata_db_mysql::MySqlTccResourceManager;
use seata_protocol::{transaction::BranchRegisterRequest, BranchType, ResultCode};
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResource};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned MySQL and explicit context-version migration"]
async fn full_context_reports_keep_original_receipt_and_recover_old_pending_with_new_callback() {
    let mut cases = Vec::new();
    for (immediate, precision_only) in [(false, false), (true, false), (false, true)] {
        for phase in [BranchPhase::Commit, BranchPhase::Rollback] {
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
            let adapter = Arc::new(
                MySqlTccResourceManager::new(
                    source.clone(),
                    "tcc-original-action".into(),
                    Arc::new(action::TccDynamicAction {
                        immediate,
                        precision_only,
                        catch_report_error: false,
                        failure_mode: None,
                    }),
                )
                .unwrap()
                .with_context_reporting(),
            );
            let name = format!("dynamic-{immediate}-{precision_only}-{phase:?}");
            let xid = format!("tcc:{name}");
            let branch = 9501;
            let original = BranchIdentity::new(
                xid.clone(),
                branch,
                adapter.key().clone(),
                Some(if precision_only {r#"{"actionContext":{"useTCCFence":true,"id":1,"amount":7,"precise":1.23000000000000000000000000001}}"#.into()} else {fixture::app(1)}),
            )
            .unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (rm, store) = manager(adapter.clone(), &listener, &name).await;
            let id = original.clone();
            let (prepared, ready) = tokio::sync::oneshot::channel();
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
                if phase == BranchPhase::Commit {
                    assert_eq!(
                        peer::callback(&mut socket, -401, &id, phase).await,
                        (ResultCode::Success, 6)
                    );
                }
                prepared.send(()).unwrap();
                let mut reports = Vec::new();
                for _ in 0..if immediate { 2 } else { 1 } {
                    let (request, app) = peer::read_context_report(&mut socket, &id).await;
                    peer::reply_context_report(&mut socket, request, ResultCode::Success).await;
                    reports.push(app);
                }
                reports
            });
            let receipt = rm
                .client()
                .branch_register_with_receipt(BranchRegisterRequest {
                    xid: Some(xid),
                    branch_type: BranchType::Tcc,
                    resource_id: Some(adapter.key().resource_id().into()),
                    lock_key: None,
                    application_data: original.application_data().map(str::to_owned),
                })
                .await
                .unwrap();
            ready.await.unwrap();
            assert_eq!(
                store.lease(original.clone(), phase).await.unwrap().state(),
                if phase == BranchPhase::Commit {
                    Some(BranchRecordState::Pending)
                } else {
                    None
                }
            );
            let returned = adapter
                .try_registered_with_context(&rm.client(), receipt.clone())
                .await
                .unwrap();
            assert_eq!(returned.identity(), receipt.identity());
            let reports = server.await.unwrap();
            assert_eq!(reports.len(), if immediate { 2 } else { 1 });
            for app in &reports {
                assert!(app.contains(if precision_only {
                    "1.23000000000000000000000000002"
                } else {
                    "1.23000000000000000000000000001"
                }));
            }
            assert_eq!(fixture::business(&mut admin).await, vec![(1, 7)]);
            rm.shutdown().await.unwrap();
            drop(store);
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let fresh = Arc::new(
                MySqlTccResourceManager::new(
                    source.clone(),
                    "tcc-original-action".into(),
                    Arc::new(fixture::Action::default()),
                )
                .unwrap()
                .with_context_reporting(),
            );
            let (fresh_rm, reopened) = manager(fresh, &listener, &name).await;
            let current = BranchIdentity::new(
                original.xid().into(),
                branch,
                original.resource().clone(),
                Some(reports.last().unwrap().clone()),
            )
            .unwrap();
            let server = tokio::spawn(async move {
                let mut socket = peer::bootstrap(&listener, current.resource().resource_id()).await;
                for request in [-402, -403] {
                    assert_eq!(
                        peer::callback(&mut socket, request, &current, phase).await,
                        (
                            ResultCode::Success,
                            if phase == BranchPhase::Commit { 5 } else { 8 }
                        )
                    );
                }
            });
            server.await.unwrap();
            assert_eq!(
                reopened
                    .lease(original.clone(), phase)
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Completed)
            );
            assert_eq!(
                fixture::business(&mut admin).await,
                if phase == BranchPhase::Commit {
                    vec![(1, if precision_only { 14 } else { 18 })]
                } else {
                    vec![]
                }
            );
            let fence_data: Vec<u8> =
                sqlx::query_scalar("SELECT application_data FROM seata_rs_tcc_fence_v1")
                    .fetch_one(&mut admin)
                    .await
                    .unwrap();
            assert_eq!(fence_data, original.application_data().unwrap().as_bytes());
            let versions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_context_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(versions, reports.len() as i64 + 1);
            fresh_rm.shutdown().await.unwrap();
            source.close().await.unwrap();
            cases.push(serde_json::json!({"immediate":immediate,"precision_only":precision_only,"phase":format!("{phase:?}"),"xid":original.xid(),"branch_id":branch,"original_app":original.application_data(),"reports":reports,"old_pending":phase==BranchPhase::Commit,"journal":"Completed","versions":versions}));
        }
    }
    fixture::record(
        "dynamic-reports",
        serde_json::json!({"cases":cases,"whole_plan_complete":false}),
    );
}

#[tokio::test]
#[ignore = "requires owned MySQL and original report failure/cancellation injection"]
async fn report_rejection_loss_caught_failure_and_cancellation_never_commit_try() {
    let mut cases = Vec::new();
    for (index, mode) in [
        "rejected",
        "lost",
        "caught",
        "cancelled",
        "business_false",
        "business_error",
    ]
    .into_iter()
    .enumerate()
    {
        let (mut admin, seed, _) = fixture::seed().await;
        seed.close().await.unwrap();
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
        let source = seata_db_mysql::MySqlDataSource::connect(
            fixture::owned_mysql::options(None)
                .operation_timeout(std::time::Duration::from_secs(8))
                .unwrap(),
        )
        .await
        .unwrap();
        let adapter = Arc::new(
            MySqlTccResourceManager::new(
                source.clone(),
                "tcc-original-action".into(),
                Arc::new(action::TccDynamicAction {
                    immediate: !mode.starts_with("business_"),
                    precision_only: false,
                    catch_report_error: mode == "caught",
                    failure_mode: if mode == "business_false" {
                        Some("false")
                    } else if mode == "business_error" {
                        Some("error")
                    } else {
                        None
                    },
                }),
            )
            .unwrap()
            .with_context_reporting(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let name = format!("dynamic-error-{mode}");
        let xid = format!("tcc:{name}");
        let branch = 9600 + index as i64;
        let original = BranchIdentity::new(
            xid.clone(),
            branch,
            adapter.key().clone(),
            Some(fixture::app(1)),
        )
        .unwrap();
        let (rm, store) = manager(adapter.clone(), &listener, &name).await;
        let id = original.clone();
        let (sent, seen) = tokio::sync::oneshot::channel();
        let (resume, resumed) = tokio::sync::oneshot::channel();
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
            let (request, app) = peer::read_context_report(&mut socket, &id).await;
            sent.send(()).unwrap();
            if mode == "cancelled" {
                resumed.await.unwrap();
                peer::reply_context_report(&mut socket, request, ResultCode::Success).await;
                let current = BranchIdentity::new(
                    id.xid().into(),
                    branch,
                    id.resource().clone(),
                    Some(app.clone()),
                )
                .unwrap();
                for request in [-501, -502] {
                    assert_eq!(
                        peer::callback(&mut socket, request, &current, BranchPhase::Rollback).await,
                        (ResultCode::Success, 8)
                    );
                }
            } else if mode != "lost" {
                peer::reply_context_report(
                    &mut socket,
                    request,
                    if mode.starts_with("business_") {
                        ResultCode::Success
                    } else {
                        ResultCode::Failed
                    },
                )
                .await;
                peer::await_eof(&mut socket).await;
            }
            app
        });
        let receipt = rm
            .client()
            .branch_register_with_receipt(BranchRegisterRequest {
                xid: Some(xid),
                branch_type: BranchType::Tcc,
                resource_id: Some(adapter.key().resource_id().into()),
                lock_key: None,
                application_data: original.application_data().map(str::to_owned),
            })
            .await
            .unwrap();
        let mut unknown = false;
        let report_app;
        if mode == "cancelled" {
            let resource = adapter.clone();
            let client = rm.client();
            let retained = receipt.clone();
            let task = tokio::spawn(async move {
                resource
                    .try_registered_with_context(&client, retained)
                    .await
            });
            seen.await.unwrap();
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            resume.send(()).unwrap();
            report_app = server.await.unwrap();
            assert_eq!(
                store
                    .lease(original.clone(), BranchPhase::Rollback)
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Completed)
            );
            assert!(adapter
                .try_registered_with_context(&rm.client(), receipt.clone())
                .await
                .is_err());
            rm.shutdown().await.unwrap();
            drop(store);
        } else {
            let error = adapter
                .try_registered_with_context(&rm.client(), receipt.clone())
                .await
                .unwrap_err();
            seen.await.unwrap();
            assert_eq!(
                error.registered_branch().unwrap().identity(),
                receipt.identity()
            );
            if mode.starts_with("business_") {
                assert!(matches!(
                    error,
                    seata_db_mysql::MySqlTccTryError::Refused { .. }
                ));
            } else {
                let seata_db_mysql::MySqlTccTryError::ContextReport {
                    source: reason,
                    context_report_started,
                    ..
                } = error
                else {
                    panic!("typed original report error lost")
                };
                assert!(context_report_started);
                let seata_db_mysql::MySqlTccContextReportError::Rpc(reason) = *reason else {
                    panic!("unexpected local report failure")
                };
                unknown = reason.is_unknown_outcome();
                assert_eq!(unknown, mode == "lost");
                if mode != "lost" {
                    assert!(matches!(
                        reason,
                        seata_rm::RmCallError::DefiniteRejected { .. }
                    ));
                }
            }
            assert!(fixture::business(&mut admin).await.is_empty());
            let fence: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(fence, 0);
            rm.shutdown().await.unwrap();
            drop(store);
            report_app = server.await.unwrap();
            let fresh = Arc::new(
                MySqlTccResourceManager::new(
                    source.clone(),
                    "tcc-original-action".into(),
                    Arc::new(fixture::Action::default()),
                )
                .unwrap()
                .with_context_reporting(),
            );
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (recovery, reopened) = manager(fresh, &listener, &name).await;
            let current = BranchIdentity::new(
                original.xid().into(),
                branch,
                original.resource().clone(),
                Some(if unknown || mode.starts_with("business_") {
                    report_app.clone()
                } else {
                    original.application_data().unwrap().into()
                }),
            )
            .unwrap();
            let server = tokio::spawn(async move {
                let mut socket = peer::bootstrap(&listener, current.resource().resource_id()).await;
                for request in [-503, -504] {
                    assert_eq!(
                        peer::callback(&mut socket, request, &current, BranchPhase::Rollback).await,
                        (ResultCode::Success, 8)
                    );
                }
            });
            server.await.unwrap();
            assert_eq!(
                reopened
                    .lease(original.clone(), BranchPhase::Rollback)
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Completed)
            );
            recovery.shutdown().await.unwrap();
        }
        assert!(fixture::business(&mut admin).await.is_empty());
        let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(status, 4);
        let versions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_context_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(versions, 2);
        source.close().await.unwrap();
        cases.push(serde_json::json!({"mode":mode,"xid":original.xid(),"branch_id":branch,"original_receipt_retained":true,"report_unknown":unknown,"report_acknowledged_before_try_result":mode.starts_with("business_"),"reports":1,"report_app":report_app,"try_business_rows":0,"fence_status":status,"versions":versions,"journal":"Completed"}));
    }
    fixture::record(
        "dynamic-report-errors",
        serde_json::json!({"cases":cases,"whole_plan_complete":false}),
    );
}
