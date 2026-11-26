//! 准备链路的拒绝／未知／早到回调／围栏竞争与取消义务。
use super::{callback_peer, first_fixture, fixture, rm_fixture};
use first_fixture::Harness;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{DbError, MySqlPrepareError, MySqlRecoveryError};
use seata_protocol::transaction::BranchRegisterResponse;
use seata_protocol::{BranchStatus, Message, ResultCode};
use seata_rm::{BranchPhase, BranchResource};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn rejected_and_lost_registration_consume_business_without_fabricating_a_receipt() {
    for unknown in [false, true] {
        let h = Harness::start(Duration::from_secs(3), Duration::from_millis(300)).await;
        let mut socket = h.accept().await;
        let tx = h.transaction().await;
        let adapter = h.adapter.clone();
        let client = h.rm.client();
        let xid = if unknown {
            "prepare:lost"
        } else {
            "prepare:rejected"
        };
        let task = tokio::spawn(async move {
            adapter
                .prepare_transaction(tx, &client, xid, Some("prepare-fixture-app".into()))
                .await
        });
        let id = first_fixture::request(&mut socket, xid).await;
        if !unknown {
            let mut response = rm_fixture::peer::success();
            response.result_code = ResultCode::Failed;
            rm_fixture::peer::write(
                &mut socket,
                rm_fixture::peer::reply(
                    id,
                    Message::BranchRegisterResponse(BranchRegisterResponse {
                        response,
                        branch_id: 9007199254740995,
                    }),
                ),
            )
            .await;
        }
        let error = task.await.unwrap().unwrap_err();
        assert!(matches!(error, MySqlPrepareError::Registration(_)));
        assert_eq!(error.is_registration_unknown(), unknown);
        assert!(error.registered_branch().is_none());
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
        fixture::save(
            if unknown {
                "prepare-registration-unknown.json"
            } else {
                "prepare-registration-rejected.json"
            },
            &serde_json::json!({"xid":xid,"registration_unknown":unknown,"receipt":false,"requests":1,"business_rows":0,"undo_rows":0,"control_rows":0}),
        );
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn early_callback_prevents_preparation_and_retains_the_genuine_late_receipt() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    let key = h.adapter.key().clone();
    let tx = h.transaction().await;
    let adapter = h.adapter.clone();
    let client = h.rm.client();
    let task = tokio::spawn(async move {
        adapter
            .prepare_transaction(
                tx,
                &client,
                "prepare:early",
                Some("prepare-fixture-app".into()),
            )
            .await
    });
    let wire = first_fixture::request(&mut socket, "prepare:early").await;
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "prepare:early",
            42,
            -151,
            Some("prepare-fixture-app"),
            BranchPhase::Rollback
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    first_fixture::positive(&mut socket, wire).await;
    let error = task.await.unwrap().unwrap_err();
    assert!(matches!(
        &error,
        MySqlPrepareError::Enrollment {
            source: MySqlRecoveryError::BeforeCommit {
                source: DbError::CallbackObserved,
                ..
            },
            ..
        }
    ));
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    assert!(!error.is_registration_unknown());
    let origin: u8 = sqlx::query_scalar("SELECT registration_origin FROM seata_rs_at_branch_v1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(origin, 2);
    assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 2, 1)));
    fixture::save(
        "prepare-early-callback.json",
        &serde_json::json!({"receipt":42,"error":"CallbackObserved","origin":2,"business_rows":0,"guard_rows":1,"terminal":1}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn existing_guard_rejects_normal_undo_after_registration_and_preserves_real_identity() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    sqlx::query("INSERT INTO undo_log VALUES(42,'prepare:guard','serializer=jackson&compressorType=NONE',?,1,now(6),now(6))").bind(b"{}".as_slice()).execute(&mut h.admin).await.unwrap();
    let tx = h.transaction().await;
    let adapter = h.adapter.clone();
    let client = h.rm.client();
    let task = tokio::spawn(async move {
        adapter
            .prepare_transaction(
                tx,
                &client,
                "prepare:guard",
                Some("prepare-fixture-app".into()),
            )
            .await
    });
    let wire = first_fixture::request(&mut socket, "prepare:guard").await;
    first_fixture::positive(&mut socket, wire).await;
    let error = task.await.unwrap().unwrap_err();
    assert!(matches!(error, MySqlPrepareError::Undo { .. }));
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    assert!(!error.is_registration_unknown());
    assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 0, 0)));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn local_validation_poison_and_sealed_undo_are_rejected_before_any_registration() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    for (xid, app) in [
        ("", None),
        (" bad", None),
        ("prepare:bad-app", Some(String::new())),
        ("prepare:too-long", Some("x".repeat(1_048_577))),
    ] {
        let tx = h.transaction().await;
        let error = h
            .adapter
            .prepare_transaction(tx, &h.rm.client(), xid, app)
            .await
            .unwrap_err();
        assert!(matches!(error, MySqlPrepareError::Local { .. }));
        assert!(error.registered_branch().is_none());
        // 等实际关闭的原租约清理完毕，再创建下一次独立本地作用域。
        wait_business_empty(&mut h.admin).await;
    }
    let mut tx = h.transaction().await;
    assert!(tx
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'duplicate')")
                .unwrap(),
            &[]
        )
        .await
        .is_err());
    assert!(matches!(
        h.adapter
            .prepare_transaction(tx, &h.rm.client(), "prepare:poison", None)
            .await,
        Err(MySqlPrepareError::Local {
            source: DbError::Poisoned
        })
    ));
    wait_business_empty(&mut h.admin).await;
    let mut tx = h.transaction().await;
    tx.flush_undo_logs("prepare:sealed", 42, MySqlJdbcProfile::Connector8Default)
        .await
        .unwrap();
    assert!(matches!(
        h.adapter
            .prepare_transaction(tx, &h.rm.client(), "prepare:sealed", None)
            .await,
        Err(MySqlPrepareError::Local {
            source: DbError::UndoSealed
        })
    ));
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
}

async fn wait_business_empty(admin: &mut sqlx::MySqlConnection) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while fixture::count(admin, "undo_work").await != 0 {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn outer_registration_deadline_and_explicit_cancellation_close_original_transaction() {
    for cancel in [false, true] {
        let h = Harness::start(Duration::from_millis(1500), Duration::from_secs(5)).await;
        let mut socket = h.accept().await;
        let tx = h.transaction().await;
        let adapter = h.adapter.clone();
        let client = h.rm.client();
        let task = tokio::spawn(async move {
            adapter
                .prepare_transaction(
                    tx,
                    &client,
                    "prepare:deadline",
                    Some("prepare-fixture-app".into()),
                )
                .await
        });
        first_fixture::request(&mut socket, "prepare:deadline").await;
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            let error = task.await.unwrap().unwrap_err();
            assert!(matches!(
                error,
                MySqlPrepareError::Deadline {
                    receipt: None,
                    registration_may_have_effect: true
                }
            ));
            assert!(error.is_registration_unknown());
        }
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
    }
}
