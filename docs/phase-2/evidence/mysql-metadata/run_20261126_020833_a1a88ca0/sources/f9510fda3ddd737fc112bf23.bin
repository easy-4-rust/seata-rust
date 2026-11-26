//! 并发、提前决定、物理能力与权限回滚的数据库验收。
use super::cases::{log, state, wait_table};
use super::{callback_peer, enrolled, enrolled_at, fixture, owned_mysql};
use seata_db_mysql::MySqlDataSource;
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchResolution, BranchResource};
use std::time::Duration;

async fn global_delete_count(admin: &mut sqlx::MySqlConnection) -> u64 {
    let (name, value): (String, String) = sqlx::query_as("SHOW GLOBAL STATUS LIKE 'Com_delete'")
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(name, "Com_delete");
    value.parse().unwrap()
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn parallel_independent_recovery_instances_serialize_one_cleanup_effect() {
    let (mut h, receipt, mut socket) = enrolled().await;
    sqlx::query("INSERT INTO undo_work VALUES(1,'parallel-fixture')")
        .execute(&mut h.admin)
        .await
        .unwrap();
    log(&mut h.admin, "rollback:xid", 42, "serializer=opaque", 0).await;
    for id in 1000..1130 {
        log(&mut h.admin, "rollback:xid", id, "branchId=42", 0).await;
    }
    log(&mut h.admin, "rollback:xid", 5000, "serializer=sentinel", 0).await;
    let second = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let third = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let second_adapter = super::recovery::adapter(second.clone());
    let third_adapter = super::recovery::adapter(third.clone());
    let before = global_delete_count(&mut h.admin).await;
    let mut blocker = owned_mysql::admin().await;
    sqlx::raw_sql("BEGIN").execute(&mut blocker).await.unwrap();
    sqlx::query("SELECT branch_id FROM undo_log WHERE branch_id=42 FOR UPDATE")
        .fetch_all(&mut blocker)
        .await
        .unwrap();
    let key = h.adapter.key().clone();
    let callback = tokio::spawn(async move {
        let outcome = callback_peer::callback(
            &mut socket,
            &key,
            "rollback:xid",
            42,
            -221,
            None,
            BranchPhase::Commit,
        )
        .await;
        (socket, outcome)
    });
    wait_table(&mut h.admin, "undo_log").await;
    let id = receipt.identity().clone();
    let one = tokio::spawn(async move { second_adapter.recover(&id, BranchPhase::Commit).await });
    let id = receipt.identity().clone();
    let two = tokio::spawn(async move { third_adapter.recover(&id, BranchPhase::Commit).await });
    let until = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='seata_rs_at_branch_v1'").fetch_one(&mut h.admin).await.unwrap();
        if waits >= 2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < until,
            "two independent recovery row waits required"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    sqlx::raw_sql("ROLLBACK")
        .execute(&mut blocker)
        .await
        .unwrap();
    let (mut socket, outcome) = callback.await.unwrap();
    assert_eq!(
        outcome,
        (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
    );
    assert_eq!(one.await.unwrap().unwrap(), BranchResolution::Applied);
    assert_eq!(two.await.unwrap().unwrap(), BranchResolution::Applied);
    let deletes = global_delete_count(&mut h.admin).await - before;
    assert_eq!(
        deletes, 131,
        "one root and 130 children must be deleted only once"
    );
    assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 1));
    second.close().await.unwrap();
    third.close().await.unwrap();
    let business: String = sqlx::query_scalar("SELECT v FROM undo_work WHERE id=1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(business, "parallel-fixture");
    assert_eq!(h.closed_counts(&mut socket).await, (1, 1, (1, 1, 0)));
    fixture::save(
        "commit-direction-parallel.json",
        &serde_json::json!({"independent_recovery_instances":2,"actual_recovery_row_waits_at_least":2,"deleted_root_and_children_once":deletes,"business":business,"remaining_sentinel_rows":1,"terminal":1}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn early_real_commit_decision_blocks_late_precommit_and_original_local_normal_is_rolled_back()
{
    let mut h =
        super::first_fixture::Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    sqlx::raw_sql(include_str!(
        "../../migrations/003_seata_rs_at_commit_decision.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    let local = h.transaction().await;
    let client = h.rm.client();
    let peer = async {
        let mut socket = h.accept().await;
        let request = super::rm_fixture::peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(seata_protocol::Message::BranchRegisterRequest(_))
        ));
        super::first_fixture::positive(&mut socket, request.id).await;
        socket
    };
    let (prepared, socket) = tokio::join!(
        h.adapter
            .prepare_transaction(local, &client, "rollback:xid", None),
        peer
    );
    let prepared = prepared.unwrap();
    let key = h.adapter.key().clone();
    let callback = tokio::spawn(async move {
        let mut socket = socket;
        let outcome = callback_peer::callback(
            &mut socket,
            &key,
            "rollback:xid",
            42,
            -222,
            None,
            BranchPhase::Commit,
        )
        .await;
        (socket, outcome)
    });
    wait_table(&mut h.admin, "undo_log").await;
    assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 0));
    let error = prepared.lock_commit_preconditions().await.unwrap_err();
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    assert!(
        matches!(
            error.database_error(),
            seata_db_mysql::DbError::RecoveryConflict
                | seata_db_mysql::DbError::Driver {
                    code: Some(1213),
                    ..
                }
        ),
        "{error:?}"
    );
    let local_deadlock = matches!(
        error.database_error(),
        seata_db_mysql::DbError::Driver {
            code: Some(1213),
            ..
        }
    );
    let (mut socket, first) = callback.await.unwrap();
    assert!(
        first == (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
            || first
                == (
                    ResultCode::Success,
                    BranchStatus::PhaseTwoCommitFailedRetryable
                )
    );
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            h.adapter.key(),
            "rollback:xid",
            42,
            -223,
            None,
            BranchPhase::Commit
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
    );
    assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 1));
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 1, 0)));
    fixture::save(
        "commit-direction-early.json",
        &serde_json::json!({"actual_local_Normal_wait":true,"durable_commit_decision_before_check":true,"late_precommit_refused":true,"local_deadlock_victim":local_deadlock,"business_rows":0,"undo_rows":0,"terminal":1,"all_unmanaged_old_attempts_fenced":false}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn malformed_commit_capability_shape_defaults_and_trigger_never_authorize_or_clean() {
    let mutations=[
        ("auth_default","ALTER TABLE seata_rs_at_branch_v1 ALTER COLUMN commit_authorized SET DEFAULT 1"),
        ("terminal_default","ALTER TABLE seata_rs_at_branch_v1 ALTER COLUMN commit_terminal SET DEFAULT 1"),
        ("auth_signed","ALTER TABLE seata_rs_at_branch_v1 MODIFY commit_authorized TINYINT NOT NULL DEFAULT 0"),
        ("terminal_nullable","ALTER TABLE seata_rs_at_branch_v1 MODIFY commit_terminal TINYINT UNSIGNED NULL DEFAULT 0"),
        ("missing_marker","ALTER TABLE seata_rs_at_branch_v1 DROP COLUMN commit_terminal"),
        ("extra_column","ALTER TABLE seata_rs_at_branch_v1 ADD unexpected INT NULL"),
        ("wrong_order","ALTER TABLE seata_rs_at_branch_v1 MODIFY commit_authorized TINYINT UNSIGNED NOT NULL DEFAULT 0 AFTER modified_at"),
        ("trigger","CREATE TRIGGER commit_forbidden BEFORE UPDATE ON seata_rs_at_branch_v1 FOR EACH ROW SET NEW.commit_terminal=NEW.commit_terminal"),
    ];
    let mut observations = Vec::new();
    for (name, sql) in mutations {
        let (mut h, receipt, mut socket) = enrolled().await;
        log(&mut h.admin, "rollback:xid", 42, "serializer=opaque", 0).await;
        sqlx::raw_sql(sql).execute(&mut h.admin).await.unwrap();
        assert_eq!(
            h.adapter
                .recover(receipt.identity(), BranchPhase::Commit)
                .await
                .unwrap(),
            BranchResolution::Unknown
        );
        assert_eq!(
            callback_peer::callback(
                &mut socket,
                h.adapter.key(),
                "rollback:xid",
                42,
                -224,
                None,
                BranchPhase::Commit
            )
            .await,
            (
                ResultCode::Failed,
                BranchStatus::PhaseTwoCommitFailedRetryable
            )
        );
        assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 0, 0)));
        observations.push(serde_json::json!({"case":name,"public_result":"Unknown","callback":"FailedRetryable","undo_rows":1,"phase":0,"rollback_terminal":0}));
    }
    fixture::save(
        "commit-direction-schema.json",
        &serde_json::json!(observations),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn terminal_update_permission_denial_rolls_back_actual_deletion_and_recovers_after_grant() {
    let (mut h, receipt, mut socket) = enrolled_at(3, Duration::from_millis(900)).await;
    log(&mut h.admin, "rollback:xid", 42, "serializer=opaque", 0).await;
    sqlx::query("INSERT INTO undo_work VALUES(1,'permission-fixture')")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let mut blocker = owned_mysql::admin().await;
    sqlx::raw_sql("BEGIN").execute(&mut blocker).await.unwrap();
    sqlx::query("SELECT branch_id FROM undo_log WHERE branch_id=42 FOR UPDATE")
        .fetch_all(&mut blocker)
        .await
        .unwrap();
    let key = h.adapter.key().clone();
    let callback = tokio::spawn(async move {
        let outcome = callback_peer::callback(
            &mut socket,
            &key,
            "rollback:xid",
            42,
            -225,
            None,
            BranchPhase::Commit,
        )
        .await;
        (socket, outcome)
    });
    wait_table(&mut h.admin, "undo_log").await;
    let (returned, outcome) = callback.await.unwrap();
    socket = returned;
    assert_eq!(
        outcome,
        (
            ResultCode::Success,
            BranchStatus::PhaseTwoCommitFailedRetryable
        )
    );
    sqlx::raw_sql("ROLLBACK")
        .execute(&mut blocker)
        .await
        .unwrap();
    assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 0));
    let database = std::env::var("SEATA_MYSQL_DATABASE").unwrap();
    assert!(
        database.starts_with("undo_")
            && database
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    for sql in [
        "CREATE USER 'commit_terminal_denied'@'%' IDENTIFIED BY 'seata-mysql-fixture-only'".to_owned(),
        "GRANT PROCESS ON *.* TO 'commit_terminal_denied'@'%'".to_owned(),
        format!("GRANT SELECT,INSERT,DELETE,TRIGGER ON `{database}`.* TO 'commit_terminal_denied'@'%'"),
        "UPDATE performance_schema.setup_consumers SET ENABLED='YES' WHERE NAME='events_statements_history_long'".to_owned(),
        "TRUNCATE TABLE performance_schema.events_statements_history_long".to_owned(),
    ] {sqlx::raw_sql(&sql).execute(&mut h.admin).await.unwrap();}
    let options = seata_db_mysql::MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        "commit_terminal_denied".into(),
        "seata-mysql-fixture-only".into(),
        database.clone(),
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap();
    let denied_source = MySqlDataSource::connect(options).await.unwrap();
    let denied = super::recovery::adapter(denied_source.clone());
    let before = global_delete_count(&mut h.admin).await;
    assert_eq!(
        denied
            .recover(receipt.identity(), BranchPhase::Commit)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    let delete_attempts = global_delete_count(&mut h.admin).await - before;
    assert_eq!(delete_attempts, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 0));
    let trace:Vec<(String,Option<String>,u64,u64,u64,u64)>=sqlx::query_as("SELECT CAST(EVENT_NAME AS CHAR CHARACTER SET utf8mb4),CAST(SQL_TEXT AS CHAR CHARACTER SET utf8mb4),CAST(MYSQL_ERRNO AS UNSIGNED),CAST(ROWS_AFFECTED AS UNSIGNED),CAST(THREAD_ID AS UNSIGNED),CAST(TIMER_START AS UNSIGNED) FROM performance_schema.events_statements_history_long ORDER BY TIMER_START")
        .fetch_all(&mut h.admin).await.unwrap();
    fixture::save(
        "commit-direction-permission-trace.json",
        &serde_json::json!(trace),
    );
    assert!(
        trace.iter().any(|v| v.2 == 0
            && v.3 == 1
            && ((v.0.ends_with("/delete") || v.0 == "statement/com/Execute")
                || v.1
                    .as_deref()
                    .is_some_and(|s| s.starts_with("DELETE FROM undo_log")))),
        "actual successful DELETE event required: {trace:?}"
    );
    assert!(
        trace.iter().any(|v| v.2 == 1142
            && ((v.0.ends_with("/update") || v.0 == "statement/com/Prepare")
                || v.1.as_deref().is_some_and(
                    |s| s.starts_with("UPDATE seata_rs_at_branch_v1 SET commit_terminal")
                ))),
        "actual terminal privilege error required: {trace:?}"
    );
    let deletes: Vec<_> = trace
        .iter()
        .filter(|v| v.2 == 0 && v.3 == 1 && v.0 == "statement/com/Execute")
        .collect();
    let denied_prepares: Vec<_> = trace
        .iter()
        .filter(|v| v.2 == 1142 && v.0 == "statement/com/Prepare")
        .collect();
    assert_eq!(deletes.len(), 1, "exactly one successful DELETE execution");
    assert_eq!(
        denied_prepares.len(),
        1,
        "exactly one denied terminal preparation"
    );
    assert_eq!(
        deletes[0].4, denied_prepares[0].4,
        "same actual recovery thread"
    );
    assert!(
        deletes[0].5 < denied_prepares[0].5,
        "delete precedes terminal denial"
    );
    let unchanged_business: String = sqlx::query_scalar("SELECT v FROM undo_work WHERE id=1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(unchanged_business, "permission-fixture");
    sqlx::raw_sql(&format!(
        "GRANT UPDATE ON `{database}`.seata_rs_at_branch_v1 TO 'commit_terminal_denied'@'%'"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(
        denied
            .recover(receipt.identity(), BranchPhase::Commit)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 1));
    denied_source.close().await.unwrap();
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            h.adapter.key(),
            "rollback:xid",
            42,
            -226,
            None,
            BranchPhase::Commit
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
    );
    assert_eq!(h.closed_counts(&mut socket).await, (1, 0, (1, 1, 0)));
    fixture::save(
        "commit-direction-permission.json",
        &serde_json::json!({"actual_delete_attempts":delete_attempts,"successful_delete_then_terminal_errno":1142,"undo_preserved_before_grant":1,"authorization_retained":true,"same_identity_after_grant":"Applied","business_rows":1,"terminal":1}),
    );
}
