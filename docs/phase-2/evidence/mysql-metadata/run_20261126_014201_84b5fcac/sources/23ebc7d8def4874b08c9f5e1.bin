//! 准备错误的原 TCP 失败报告与取消后单次门闩，不依赖 MySQL 数据库。
#![allow(clippy::unwrap_used)]
use crate::data_source_manager::rollback_sqlx_tests::rm_fixture as fixture;
use crate::mysql_transaction_preparer::MySqlTransactionPreparer;
use crate::{
    DbError, MySqlFailureReportGuard, MySqlPrepareError, MySqlPrepareFailureReportError,
    MySqlPrepareReportedError, MySqlPreparedTransaction,
};
use fixture::{manager, peer, registered};
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchRegisterRequest, BranchRegisterResponse,
    BranchReportResponse,
};
use seata_protocol::{BranchStatus, BranchType, Message, ResultCode, TransactionExceptionCode};
use seata_rm::RmCallError;
use std::time::Duration;
use tokio::net::TcpListener;

fn request() -> BranchRegisterRequest {
    BranchRegisterRequest {
        xid: Some("prepare:failed-report".into()),
        branch_type: BranchType::At,
        resource_id: Some("mysql://exact-resource".into()),
        lock_key: Some("orders:1".into()),
        application_data: Some("original-app-data".into()),
    }
}

#[tokio::test]
async fn canceled_registration_progress_keeps_unknown_then_original_receipt() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let register = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(
                register.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: peer::success(),
                    branch_id: 42,
                }),
            ),
        )
        .await;
        assert!(
            tokio::time::timeout(Duration::from_millis(60), peer::read(&mut socket))
                .await
                .is_err()
        );
        socket
    });
    let mut slot = None;
    let mut pending = Box::pin(async {
        MySqlTransactionPreparer::record_registration_started(&mut slot);
        std::future::pending::<()>().await;
    });
    tokio::select! {
        _ = &mut pending => panic!("registration wait must remain pending"),
        _ = tokio::time::sleep(Duration::from_millis(5)) => {}
    }
    drop(pending);
    assert!(slot.as_ref().unwrap().is_registration_unknown());
    assert!(slot.as_ref().unwrap().registered_branch().is_none());
    let receipt = rm
        .client()
        .branch_register_with_receipt(request())
        .await
        .unwrap();
    let mut pending = Box::pin(async {
        MySqlTransactionPreparer::record_registered(&mut slot, &receipt);
        std::future::pending::<()>().await;
    });
    tokio::select! {
        _ = &mut pending => panic!("post-registration wait must remain pending"),
        _ = tokio::time::sleep(Duration::from_millis(5)) => {}
    }
    drop(pending);
    let progress = slot.as_ref().unwrap();
    assert_eq!(
        progress.registered_branch().unwrap().identity().branch_id(),
        42
    );
    assert!(!progress.local_rollback_confirmed());
    assert!(matches!(
        progress.report_failure_after_rollback(&rm.client()).await,
        Err(MySqlPrepareFailureReportError::NotEligible)
    ));
    let mut socket = server.await.unwrap();
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
}

#[tokio::test]
async fn orchestration_reports_confirmed_prepare_failure_and_returns_original_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let register = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(
                register.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: peer::success(),
                    branch_id: 42,
                }),
            ),
        )
        .await;
        let report = peer::read(&mut socket).await;
        match report.body {
            Some(Message::BranchReportRequest(value)) => {
                assert_eq!(value.status, BranchStatus::PhaseOneFailed);
                assert_eq!(value.branch_id, 42);
            }
            other => panic!("unexpected report: {other:?}"),
        }
        peer::write(
            &mut socket,
            peer::reply(
                report.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: peer::success(),
                }),
            ),
        )
        .await;
        assert!(
            tokio::time::timeout(Duration::from_millis(60), peer::read(&mut socket))
                .await
                .is_err()
        );
        socket
    });
    let receipt = rm
        .client()
        .branch_register_with_receipt(request())
        .await
        .unwrap();
    let prepare = MySqlPrepareError::Undo {
        receipt: Box::new(receipt),
        source: DbError::UndoSealed,
        rollback: Ok(()),
        report_guard: MySqlFailureReportGuard::new(),
    };
    let mut slot = None;
    let result = MySqlPrepareReportedError::complete(
        Err::<MySqlPreparedTransaction, _>(prepare),
        &mut slot,
        &rm.client(),
    )
    .await;
    let error = result.unwrap_err();
    assert!(matches!(
        error.prepare_error(),
        MySqlPrepareError::Undo { .. }
    ));
    assert!(error.prepare_error().failure_report_started());
    assert!(error.report_result().unwrap().is_ok());
    assert!(slot.is_none());
    let unconfirmed = MySqlPrepareError::Undo {
        receipt: Box::new(error.prepare_error().registered_branch().unwrap().clone()),
        source: DbError::UndoSealed,
        rollback: Err(Box::new(DbError::Closed)),
        report_guard: MySqlFailureReportGuard::new(),
    };
    let result = MySqlPrepareReportedError::complete(
        Err::<MySqlPreparedTransaction, _>(unconfirmed),
        &mut slot,
        &rm.client(),
    )
    .await;
    let unconfirmed = result.unwrap_err();
    assert!(unconfirmed.report_result().is_none());
    assert!(!unconfirmed.prepare_error().failure_report_started());
    assert!(slot.is_none());
    let mut socket = server.await.unwrap();
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
}

#[tokio::test]
async fn confirmed_rollback_reports_original_identity_only_once() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let register = peer::read(&mut socket).await;
        assert_eq!(
            register.body,
            Some(Message::BranchRegisterRequest(request()))
        );
        peer::write(
            &mut socket,
            peer::reply(
                register.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: peer::success(),
                    branch_id: 42,
                }),
            ),
        )
        .await;
        let report = peer::read(&mut socket).await;
        match report.body {
            Some(Message::BranchReportRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("prepare:failed-report"));
                assert_eq!(value.branch_id, 42);
                assert_eq!(value.status, BranchStatus::PhaseOneFailed);
                assert_eq!(value.resource_id.as_deref(), Some("mysql://exact-resource"));
                assert_eq!(value.application_data.as_deref(), Some("original-app-data"));
                assert_eq!(value.branch_type, BranchType::At);
            }
            other => panic!("unexpected report: {other:?}"),
        }
        peer::write(
            &mut socket,
            peer::reply(
                report.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: peer::success(),
                }),
            ),
        )
        .await;
        assert!(
            tokio::time::timeout(Duration::from_millis(60), peer::read(&mut socket))
                .await
                .is_err()
        );
        socket
    });
    let receipt = rm
        .client()
        .branch_register_with_receipt(request())
        .await
        .unwrap();
    let error = MySqlPrepareError::Undo {
        receipt: Box::new(receipt),
        source: DbError::UndoSealed,
        rollback: Ok(()),
        report_guard: MySqlFailureReportGuard::new(),
    };
    assert!(error.local_rollback_confirmed());
    assert!(!error.failure_report_started());
    let response = error
        .report_failure_after_rollback(&rm.client())
        .await
        .unwrap();
    assert_eq!(
        response.response.result_code,
        seata_protocol::ResultCode::Success
    );
    assert!(error.failure_report_started());
    assert!(matches!(
        error.report_failure_after_rollback(&rm.client()).await,
        Err(MySqlPrepareFailureReportError::AlreadyAttempted)
    ));
    let unconfirmed = MySqlPrepareError::Undo {
        receipt: Box::new(error.registered_branch().unwrap().clone()),
        source: DbError::UndoSealed,
        rollback: Err(Box::new(DbError::Closed)),
        report_guard: MySqlFailureReportGuard::new(),
    };
    assert!(matches!(
        unconfirmed
            .report_failure_after_rollback(&rm.client())
            .await,
        Err(MySqlPrepareFailureReportError::NotEligible)
    ));
    assert!(!unconfirmed.failure_report_started());
    let mut socket = server.await.unwrap();
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
}

#[tokio::test]
async fn canceled_report_wait_retains_receipt_and_refuses_replay() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let (seen, observed) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let register = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(
                register.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: peer::success(),
                    branch_id: 42,
                }),
            ),
        )
        .await;
        let report = peer::read(&mut socket).await;
        assert!(matches!(report.body, Some(Message::BranchReportRequest(_))));
        seen.send(()).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(60), peer::read(&mut socket))
                .await
                .is_err()
        );
        socket
    });
    let receipt = rm
        .client()
        .branch_register_with_receipt(request())
        .await
        .unwrap();
    let error = MySqlPrepareError::Undo {
        receipt: Box::new(receipt),
        source: DbError::UndoSealed,
        rollback: Ok(()),
        report_guard: MySqlFailureReportGuard::new(),
    };
    let client = rm.client();
    let mut slot = None;
    let mut pending = Box::pin(MySqlPrepareReportedError::complete(
        Err::<MySqlPreparedTransaction, _>(error),
        &mut slot,
        &client,
    ));
    tokio::select! {
        _ = &mut pending => panic!("report must still await the missing response"),
        _ = observed => {}
    }
    drop(pending);
    let error = slot.take().unwrap();
    assert!(error.failure_report_started());
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    assert!(matches!(
        error.report_failure_after_rollback(&client).await,
        Err(MySqlPrepareFailureReportError::AlreadyAttempted)
    ));
    let mut socket = server.await.unwrap();
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
}

#[tokio::test]
async fn tc_rejected_failure_report_keeps_original_response_and_no_replay() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let register = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(
                register.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: peer::success(),
                    branch_id: 42,
                }),
            ),
        )
        .await;
        let report = peer::read(&mut socket).await;
        match report.body {
            Some(Message::BranchReportRequest(value)) => {
                assert_eq!(value.status, BranchStatus::PhaseOneFailed);
                assert_eq!(value.branch_id, 42);
            }
            other => panic!("unexpected report: {other:?}"),
        }
        peer::write(
            &mut socket,
            peer::reply(
                report.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: AbstractTransactionResponse {
                        result_code: ResultCode::Failed,
                        msg: Some("private report failure".into()),
                        transaction_exception_code: TransactionExceptionCode::BranchReportFailed,
                    },
                }),
            ),
        )
        .await;
        assert!(
            tokio::time::timeout(Duration::from_millis(60), peer::read(&mut socket))
                .await
                .is_err()
        );
        socket
    });
    let receipt = rm
        .client()
        .branch_register_with_receipt(request())
        .await
        .unwrap();
    let error = MySqlPrepareError::Undo {
        receipt: Box::new(receipt),
        source: DbError::UndoSealed,
        rollback: Ok(()),
        report_guard: MySqlFailureReportGuard::new(),
    };
    let failure = error
        .report_failure_after_rollback(&rm.client())
        .await
        .unwrap_err();
    assert!(!failure.is_unknown_outcome());
    assert!(!format!("{failure:?}").contains("private report failure"));
    match failure {
        MySqlPrepareFailureReportError::Call(RmCallError::DefiniteRejected {
            response, ..
        }) => match response.body {
            Some(Message::BranchReportResponse(value)) => {
                assert_eq!(value.response.result_code, ResultCode::Failed);
                assert_eq!(
                    value.response.transaction_exception_code,
                    TransactionExceptionCode::BranchReportFailed
                );
            }
            other => panic!("unexpected response: {other:?}"),
        },
        other => panic!("unexpected failure: {other:?}"),
    }
    assert!(error.failure_report_started());
    assert!(matches!(
        error.report_failure_after_rollback(&rm.client()).await,
        Err(MySqlPrepareFailureReportError::AlreadyAttempted)
    ));
    let mut socket = server.await.unwrap();
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
}
