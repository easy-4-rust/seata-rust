//! 同一已消费分支的正常补偿、回滚终态和晚到写入围栏。
use super::{callback_peer, fixture, owned_mysql, prepared, rm_fixture, Harness};
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{DbError, UndoFenceObservation};
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchRecordState, BranchResource};
use sqlx::Connection;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn managed_rollback_fences_a_stale_writer_after_compensating_normal_undo() {
    let fault = std::env::var("SEATA_MANAGED_ROLLBACK_FAULT").ok();
    let store = rm_fixture::journal().await;
    let budget = if fault.as_deref() == Some("delay_ok") {
        Duration::from_millis(1500)
    } else {
        Duration::from_secs(3)
    };
    let mut h = Harness::start_with_store(budget, Duration::from_secs(2), store.clone()).await;
    if fault.is_some() {
        h.admin = owned_mysql::admin_at(
            std::env::var("SEATA_MANAGED_ROLLBACK_ADMIN_PORT")
                .unwrap()
                .parse()
                .unwrap(),
        )
        .await;
    }
    for migration in [
        include_str!("../../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../../migrations/004_seata_rs_at_first_phase_witness.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let xid = "commit-check:consumed-guard";
    let (prepared, mut peer) = prepared(&h, xid).await;
    let receipt = prepared.registered_branch().unwrap().clone();
    fixture::dirty(&mut h.admin).await;
    let undo: (String, Vec<u8>) =
        sqlx::query_as("SELECT context,rollback_info FROM undo_log WHERE xid=? AND branch_id=42")
            .bind(xid)
            .fetch_one(&mut h.admin)
            .await
            .unwrap();
    prepared.rollback().await.unwrap();
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut h.admin)
        .await
        .unwrap();
    // 管理夹具提交真实准备产生的 Undo 正样本及历史见证。
    let mut seed = h.admin.begin().await.unwrap();
    sqlx::query("INSERT INTO undo_work VALUES(1,'prepared_fixture')")
        .execute(&mut *seed)
        .await
        .unwrap();
    sqlx::query("INSERT INTO undo_log VALUES(42,?,?,?,0,now(6),now(6))")
        .bind(xid)
        .bind(undo.0)
        .bind(undo.1)
        .execute(&mut *seed)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE seata_rs_at_branch_v1 SET first_phase_committed=1 WHERE xid=? AND branch_id=42",
    )
    .bind(xid.as_bytes())
    .execute(&mut *seed)
    .await
    .unwrap();
    seed.commit().await.unwrap();

    // 旧作用域已开始且写入不同业务主键，所以可与本次补偿同时存在。
    let mut stale = h.source.begin().await.unwrap();
    let input =
        SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(2,'stale-local')")
            .unwrap();
    assert_eq!(stale.execute(&input, &[]).await.unwrap(), 1);
    let first_reply = callback_peer::callback(
        &mut peer,
        h.adapter.key(),
        xid,
        42,
        -480,
        Some("prepare-fixture-app"),
        BranchPhase::Rollback,
    )
    .await;
    let sql = "SELECT log_status,context,rollback_info,CAST(log_created AS CHAR),CAST(log_modified AS CHAR) FROM undo_log WHERE xid=? AND branch_id=42";
    type Guard = (i32, String, Vec<u8>, String, String);
    let mut persisted_before_retry = None;
    if let Some(fault) = fault.as_deref() {
        assert_eq!(
            first_reply,
            (
                ResultCode::Success,
                BranchStatus::PhaseTwoRollbackFailedRetryable
            )
        );
        assert_eq!(
            store
                .lease(receipt.identity().clone(), BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Pending)
        );
        let committed = matches!(fault, "drop_ok" | "delay_ok");
        assert!(committed || fault == "block_commit");
        let flags: (u8,u8,u8) = sqlx::query_as(
            "SELECT phase,terminal,first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42")
            .bind(xid.as_bytes()).fetch_one(&mut h.admin).await.unwrap();
        let row: Guard = sqlx::query_as(sql)
            .bind(xid)
            .fetch_one(&mut h.admin)
            .await
            .unwrap();
        let business = fixture::count(&mut h.admin, "undo_work").await;
        assert_eq!(flags, (2, u8::from(committed), 1));
        assert_eq!(row.0, i32::from(committed));
        assert_eq!(business, i64::from(!committed));
        if committed {
            persisted_before_retry = Some(row);
        }
        fixture::save(
            "managed-rollback-fault-before-retry.json",
            &serde_json::json!({
                "fault":fault,"first_reply":"PhaseTwoRollbackFailedRetryable","journal":"Pending",
                "flags":flags,"undo_status":i32::from(committed),"business_rows":business,"xid":xid,"branch_id":42
            }),
        );
        let observation = std::path::PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap())
            .join("managed-rollback-fault-before-retry.ack");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !observation.is_file() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "missing independent SQL observation before retry"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            callback_peer::callback(
                &mut peer,
                h.adapter.key(),
                xid,
                42,
                -482,
                Some("prepare-fixture-app"),
                BranchPhase::Rollback
            )
            .await,
            (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
        );
    } else {
        assert_eq!(
            first_reply,
            (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
        );
    }
    assert_eq!(
        store
            .lease(receipt.identity().clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    let result = stale
        .flush_undo_logs(xid, 42, MySqlJdbcProfile::Connector8Default)
        .await;
    assert!(
        matches!(
            result,
            Err(DbError::Driver {
                code: Some(1062),
                ..
            })
        ),
        "late Normal should be rejected by the retained guard: {result:?}"
    );
    stale.rollback().await.unwrap();
    let first: Guard = sqlx::query_as(sql)
        .bind(xid)
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    if let Some(before) = persisted_before_retry {
        assert_eq!(before, first);
    }
    assert_eq!(
        (first.0, first.1.as_str(), first.2.as_slice()),
        (
            1,
            "serializer=jackson&compressorType=NONE",
            b"{}".as_slice()
        )
    );
    assert_eq!(
        callback_peer::callback(
            &mut peer,
            h.adapter.key(),
            xid,
            42,
            -481,
            Some("prepare-fixture-app"),
            BranchPhase::Rollback
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    // 迟到提交不得删除已完成回滚的原围栏或改写已持久方向。
    assert_eq!(
        callback_peer::callback(
            &mut peer,
            h.adapter.key(),
            xid,
            42,
            -483,
            Some("prepare-fixture-app"),
            BranchPhase::Commit
        )
        .await,
        (
            ResultCode::Failed,
            BranchStatus::PhaseTwoCommitFailedUnretryable
        )
    );
    let direction: (u8, u8, u8, u8) = sqlx::query_as(
        "SELECT phase,terminal,commit_authorized,commit_terminal
         FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42",
    )
    .bind(xid.as_bytes())
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(direction, (2, 1, 0, 0));
    assert_eq!(
        h.source.fence_undo_log(xid, 42).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: false }
    );
    let second: Guard = sqlx::query_as(sql)
        .bind(xid)
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(first, second);
    assert!(h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    let (business, undo, state) = h.closed_counts(&mut peer).await;
    assert_eq!((business, undo, state), (0, 1, (1, 2, 1)));
    fixture::save(
        "managed-rollback-fence.json",
        &serde_json::json!({
            "xid":xid,"branch_id":42,"actual_late_normal_error":1062,"business_after_rollback":business,
            "guard_status":first.0,"guard_context":first.1,"guard_body":first.2,
            "repeat_callback_preserved_guard":true,"first_phase_history_preserved":true,"recovery_state":state
        }),
    );
}
