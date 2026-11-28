//! 组合成功报告的错误层级必须保留本地提交事实。
use super::{first_fixture::Harness, fixture, prepared, rm_fixture};
use seata_db_mysql::{MySqlPhaseOneOperation as Phase, MySqlPhaseOneOutcome as Outcome};
use seata_protocol::{transaction::BranchReportResponse, BranchStatus, Message, ResultCode};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn rejected_success_report_preserves_confirmed_local_commit_stage() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (value, mut socket) = prepared(&h, "phase-metrics:report-rejected").await;
    let checked = value.lock_commit_preconditions().await.unwrap();
    let peer = tokio::spawn(async move {
        let request = rm_fixture::peer::read(&mut socket).await;
        assert!(
            matches!(request.body, Some(Message::BranchReportRequest(ref report)) if report.status == BranchStatus::PhaseOneDone)
        );
        let mut response = rm_fixture::peer::success();
        response.result_code = ResultCode::Failed;
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                request.id,
                Message::BranchReportResponse(BranchReportResponse { response }),
            ),
        )
        .await;
        socket
    });
    let error = checked
        .commit_changed_and_report(&h.adapter, &h.rm.client())
        .await
        .unwrap_err();
    assert!(error.local_commit_confirmed());
    assert!(!error.is_unknown_outcome());
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    h.assert_phase(Phase::CommitChanged, Outcome::ChangedCommitted, 1);
    h.assert_phase(Phase::CommitAndReport, Outcome::ReportFailed, 1);
    h.assert_phase(Phase::CommitAndReport, Outcome::ReportLocalFailed, 0);
    let mut socket = peer.await.unwrap();
    assert_eq!(h.closed_counts(&mut socket).await, (1, 1, (1, 0, 0)));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn unregistered_combined_commit_is_local_rejection_without_report() {
    let h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    let transaction = h.source.begin().await.unwrap();
    let prepared = h
        .adapter
        .prepare_transaction(transaction, &h.rm.client(), "phase-metrics:empty", None)
        .await
        .unwrap();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    let error = checked
        .commit_changed_and_report(&h.adapter, &h.rm.client())
        .await
        .unwrap_err();
    assert!(!error.local_commit_confirmed());
    assert!(!error.is_unknown_outcome());
    assert!(error.registered_branch().is_none());
    h.assert_phase(Phase::Prepare, Outcome::PreparedUnregistered, 1);
    h.assert_phase(Phase::CommitChanged, Outcome::ChangedNotEligible, 1);
    h.assert_phase(Phase::CommitAndReport, Outcome::ReportLocalFailed, 1);
    assert!(tokio::time::timeout(
        Duration::from_millis(50),
        rm_fixture::peer::read(&mut socket)
    )
    .await
    .is_err());
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
}
