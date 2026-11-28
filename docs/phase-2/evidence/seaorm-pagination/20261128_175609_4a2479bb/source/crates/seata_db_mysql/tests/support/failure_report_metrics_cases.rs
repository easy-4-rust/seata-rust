//! 真实准备错误的源归属、单次失败报告、取消及组合入口的分层计数。
use super::{first_fixture, first_fixture::Harness, fixture, rm_fixture};
use seata_db_mysql::{
    MySqlPhaseOneOperation as Op, MySqlPhaseOneOutcome as Out, MySqlPrepareError,
    MySqlPrepareFailureReportError,
};
use seata_protocol::{transaction::BranchReportResponse, BranchStatus, Message, ResultCode};
use std::time::Duration;

async fn conflicting_undo(h: &mut Harness, xid: &str) {
    sqlx::query("INSERT INTO undo_log VALUES(42,?,'serializer=jackson&compressorType=NONE',?,1,now(6),now(6))").bind(xid).bind(b"{}".as_slice()).execute(&mut h.admin).await.unwrap();
}
async fn report(socket: &mut tokio::net::TcpStream, xid: &str) -> i32 {
    let request = rm_fixture::peer::read(socket).await;
    match request.body {
        Some(Message::BranchReportRequest(value)) => {
            assert_eq!(value.xid.as_deref(), Some(xid));
            assert_eq!(value.branch_id, 42);
            assert_eq!(value.status, BranchStatus::PhaseOneFailed);
            assert_eq!(
                value.application_data.as_deref(),
                Some("prepare-fixture-app")
            );
        }
        other => panic!("unexpected failure report {other:?}"),
    }
    request.id
}
async fn finish(h: Harness, name: &str) {
    let metrics = h.source.phase_one_metrics_sampler();
    h.rm.shutdown().await.unwrap();
    h.source.close().await.unwrap();
    assert!(Op::ALL.iter().all(|op| metrics.snapshot(*op).active == 0));
    fixture::save(
        name,
        &first_fixture::phase_one_metrics_snapshot::value(&metrics),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn explicit_original_prepare_error_preserves_report_outcomes_and_blocks_replay() {
    for mode in ["observed", "rejected", "lost", "cancelled"] {
        let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
        let xid = "failure-metrics:explicit";
        conflicting_undo(&mut h, xid).await;
        let mut socket = h.accept().await;
        let tx = h.transaction().await;
        let client = h.rm.client();
        let registration = async {
            let id = first_fixture::request(&mut socket, xid).await;
            first_fixture::positive(&mut socket, id).await;
        };
        let (result, ()) = tokio::join!(
            h.adapter
                .prepare_transaction(tx, &client, xid, Some("prepare-fixture-app".into())),
            registration
        );
        let error = result.unwrap_err();
        assert!(matches!(error, MySqlPrepareError::Undo { .. }));
        assert!(error.local_rollback_confirmed());
        assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
        assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
        let mut pending = Box::pin(error.report_failure_after_rollback(&client));
        let id = tokio::select! {value=&mut pending=>panic!("failure report completed before reply: {value:?}"),id=report(&mut socket,xid)=>id};
        assert_eq!(
            h.source
                .phase_one_metrics_sampler()
                .snapshot(Op::PrepareFailureReport)
                .active,
            1
        );
        let outcome = match mode {
            "cancelled" => {
                drop(pending);
                Out::Cancelled
            }
            "lost" => {
                drop(socket);
                assert!(pending.await.unwrap_err().is_unknown_outcome());
                Out::FailureReportUnknown
            }
            _ => {
                let mut response = rm_fixture::peer::success();
                if mode == "rejected" {
                    response.result_code = ResultCode::Failed;
                }
                rm_fixture::peer::write(
                    &mut socket,
                    rm_fixture::peer::reply(
                        id,
                        Message::BranchReportResponse(BranchReportResponse { response }),
                    ),
                )
                .await;
                let result = pending.await;
                if mode == "rejected" {
                    assert!(!result.unwrap_err().is_unknown_outcome());
                } else {
                    result.unwrap();
                }
                drop(socket);
                if mode == "rejected" {
                    Out::FailureReportFailed
                } else {
                    Out::FailureReportObserved
                }
            }
        };
        h.assert_phase(Op::PrepareFailureReport, outcome, 1);
        // 计数能力随原错误存活；源关闭后重复门闩仍拒绝，不能发第二个报告。
        h.source.close().await.unwrap();
        assert!(error.failure_report_started());
        assert!(matches!(
            error.report_failure_after_rollback(&client).await,
            Err(MySqlPrepareFailureReportError::AlreadyAttempted)
        ));
        h.assert_phase(
            Op::PrepareFailureReport,
            Out::FailureReportAlreadyAttempted,
            1,
        );
        finish(h, &format!("explicit-{mode}.json")).await;
    }
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn automatic_prepare_failure_cancel_keeps_original_error_and_nested_counters() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let xid = "failure-metrics:auto";
    conflicting_undo(&mut h, xid).await;
    let mut socket = h.accept().await;
    let tx = h.transaction().await;
    let client = h.rm.client();
    let mut slot = None;
    let mut pending = Box::pin(h.adapter.prepare_transaction_and_report_failure(
        tx,
        &client,
        xid,
        Some("prepare-fixture-app".into()),
        &mut slot,
    ));
    let peer = async {
        let id = first_fixture::request(&mut socket, xid).await;
        first_fixture::positive(&mut socket, id).await;
        report(&mut socket, xid).await;
    };
    tokio::select! {value=&mut pending=>panic!("withheld auto-report replied: {value:?}"),()=peer=>{}}
    drop(pending);
    let error = slot.as_ref().unwrap();
    assert!(error.local_rollback_confirmed());
    assert!(error.failure_report_started());
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    h.assert_phase(Op::PrepareAndReportFailure, Out::Cancelled, 1);
    h.assert_phase(Op::PrepareFailureReport, Out::Cancelled, 1);
    h.assert_phase(Op::Rollback, Out::RollbackConfirmed, 1);
    assert!(matches!(
        error.report_failure_after_rollback(&client).await,
        Err(MySqlPrepareFailureReportError::AlreadyAttempted)
    ));
    h.assert_phase(
        Op::PrepareFailureReport,
        Out::FailureReportAlreadyAttempted,
        1,
    );
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    drop(socket);
    finish(h, "automatic-cancelled.json").await;
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn automatic_prepare_failure_observed_has_separate_outer_and_report_results() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let xid = "failure-metrics:observed";
    conflicting_undo(&mut h, xid).await;
    let mut socket = h.accept().await;
    let tx = h.transaction().await;
    let client = h.rm.client();
    let mut slot = None;
    let peer = async {
        let id = first_fixture::request(&mut socket, xid).await;
        first_fixture::positive(&mut socket, id).await;
        let id = report(&mut socket, xid).await;
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: rm_fixture::peer::success(),
                }),
            ),
        )
        .await;
    };
    let (result, ()) = tokio::join!(
        h.adapter.prepare_transaction_and_report_failure(
            tx,
            &client,
            xid,
            Some("prepare-fixture-app".into()),
            &mut slot
        ),
        peer
    );
    let error = result.unwrap_err();
    assert!(error.prepare_error().local_rollback_confirmed());
    assert!(error.report_result().unwrap().is_ok());
    assert!(slot.is_none());
    h.assert_phase(Op::PrepareAndReportFailure, Out::FailureReportObserved, 1);
    h.assert_phase(Op::PrepareFailureReport, Out::FailureReportObserved, 1);
    drop(socket);
    finish(h, "automatic-observed.json").await;
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn empty_and_invalid_preparations_do_not_invent_failure_reports() {
    let h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    let client = h.rm.client();
    let mut slot = None;
    let tx = h.source.begin().await.unwrap();
    let never = h.adapter.prepare_transaction_and_report_failure(
        tx,
        &client,
        "failure-metrics:unpolled",
        None,
        &mut slot,
    );
    drop(never);
    assert_eq!(
        h.source
            .phase_one_metrics_sampler()
            .snapshot(Op::PrepareAndReportFailure),
        Default::default()
    );
    let value = h
        .adapter
        .prepare_transaction_and_report_failure(
            h.source.begin().await.unwrap(),
            &client,
            "failure-metrics:empty",
            None,
            &mut slot,
        )
        .await
        .unwrap();
    assert!(value
        .rollback_and_report_failure(&client)
        .await
        .unwrap()
        .is_none());
    let error = h
        .adapter
        .prepare_transaction_and_report_failure(
            h.source.begin().await.unwrap(),
            &client,
            "",
            None,
            &mut slot,
        )
        .await
        .unwrap_err();
    assert!(error.report_result().is_none());
    h.assert_phase(Op::PrepareAndReportFailure, Out::PreparedUnregistered, 1);
    h.assert_phase(
        Op::PrepareAndReportFailure,
        Out::PrepareFailureUnreported,
        1,
    );
    h.assert_phase(Op::RollbackAndReportFailure, Out::FailureReportSkipped, 1);
    assert!(tokio::time::timeout(
        Duration::from_millis(50),
        rm_fixture::peer::read(&mut socket)
    )
    .await
    .is_err());
    drop(socket);
    finish(h, "empty-invalid.json").await;
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn registered_combination_and_checked_delegate_have_one_report_scope() {
    let h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    let tx = h.transaction().await;
    let client = h.rm.client();
    let xid = "failure-metrics:registered";
    let mut slot = None;
    let peer = async {
        let id = first_fixture::request(&mut socket, xid).await;
        first_fixture::positive(&mut socket, id).await;
    };
    let (result, ()) = tokio::join!(
        h.adapter.prepare_transaction_and_report_failure(
            tx,
            &client,
            xid,
            Some("prepare-fixture-app".into()),
            &mut slot
        ),
        peer
    );
    let checked = result.unwrap().lock_commit_preconditions().await.unwrap();
    let peer = async {
        let id = report(&mut socket, xid).await;
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: rm_fixture::peer::success(),
                }),
            ),
        )
        .await;
    };
    let (result, ()) = tokio::join!(checked.rollback_and_report_failure(&client), peer);
    assert_eq!(result.unwrap().unwrap().identity().branch_id(), 42);
    h.assert_phase(Op::PrepareAndReportFailure, Out::PreparedRegistered, 1);
    h.assert_phase(Op::RollbackAndReportFailure, Out::FailureReportObserved, 1);
    drop(socket);
    finish(h, "registered-checked.json").await;
}
