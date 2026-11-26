//! 固定原 Java TC 连接中的无变化本地提交验证，默认构建可运行。

use crate::{fixture, owned_mysql, pool, registration};
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{DbError, MySqlDataSource, MySqlDataSourceManager, MySqlNoChangeCommitError};
use seata_protocol::GlobalStatus;
use seata_rm::{BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use std::{path::PathBuf, sync::Arc, time::Duration};

#[tokio::test]
#[ignore = "requires pinned Java TC and an owned real MySQL passthrough observer"]
async fn original_java_tc_no_change_commit_leaves_no_branch() {
    exercise(Scenario::Commit).await;
}

#[tokio::test]
#[ignore = "requires pinned Java TC and an owned MySQL COMMIT acknowledgment loss observer"]
async fn original_java_tc_no_change_lost_ack_remains_unknown_without_branch() {
    exercise(Scenario::AckLoss).await;
}

#[tokio::test]
#[ignore = "requires pinned Java TC and owned MySQL with its original COMMIT OK held"]
async fn original_java_tc_single_slot_pool_recovers_after_commit_timeout() {
    exercise(Scenario::PoolRecovery).await;
}

#[derive(Clone, Copy)]
enum Scenario {
    Commit,
    AckLoss,
    PoolRecovery,
}

async fn exercise(scenario: Scenario) {
    let recycle = matches!(scenario, Scenario::PoolRecovery);
    let lose_ack = !matches!(scenario, Scenario::Commit);
    let mut admin = fixture::reset().await;
    for migration in [
        include_str!("../../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../../migrations/005_seata_rs_at_resource_binding.sql"),
        include_str!("../../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    sqlx::query("INSERT INTO undo_work(id,v) VALUES(1,'untouched')")
        .execute(&mut admin)
        .await
        .unwrap();
    let port = std::env::var("SEATA_JAVA_RM_MYSQL_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let options = owned_mysql::options(Some(port)).with_java_tc_mysql_db_lock_profile();
    let options = if recycle {
        options
            .pool_limits(1, Duration::from_secs(2))
            .unwrap()
            .operation_timeout(Duration::from_secs(2))
            .unwrap()
    } else {
        options
    };
    let source = MySqlDataSource::connect(options).await.unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(adapter.key().resource_id().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let observations = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let journal = FileBranchStore::open(
        observations.join("java-no-change-branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        journal,
    )
    .build()
    .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while rm.connection_snapshot().ready() == 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC did not register RM"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let tm = TransactionManager::builder(
        pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
        registration(),
    )
    .request_timeout(Duration::from_secs(10))
    .build()
    .unwrap();
    let cfg = TransactionConfig::builder("mysql-no-change-original-tc".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut global = tm.begin(&cfg).await.unwrap();
    let xid = global.xid().clone();
    let mut local = source.begin().await.unwrap();
    let affected = local
        .execute(
            &SqlRecognizerFactory::recognize("UPDATE undo_work SET v='absent' WHERE id=99")
                .unwrap(),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(affected, 0);
    let checked = adapter
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    assert!(checked.registered_branch().is_none());
    assert_eq!(checked.undo_log_count(), 0);
    assert!(checked.require_first_phase_commit_witness().is_err());

    let result = checked.commit_no_change().await;
    let local_outcome = if lose_ack {
        let error = result.expect_err("missing original MySQL COMMIT OK must remain unknown");
        assert!(error.is_unknown_outcome());
        assert!(error.registered_branch().is_none());
        match (scenario, error) {
            (
                Scenario::PoolRecovery,
                MySqlNoChangeCommitError::Unknown {
                    source:
                        DbError::Deadline {
                            operation: "no-change COMMIT",
                        },
                },
            ) => {}
            (
                Scenario::AckLoss,
                MySqlNoChangeCommitError::Unknown {
                    source:
                        DbError::Driver {
                            operation: "no-change COMMIT",
                            ..
                        },
                },
            ) => {}
            (_, other) => {
                panic!("expected the original no-change COMMIT to remain unknown: {other:?}")
            }
        }
        "Unknown"
    } else {
        result.unwrap();
        "Confirmed"
    };
    fixture::save(
        "java-no-change-local.json",
        &serde_json::json!({
            "xid": xid.as_str(), "local_outcome": local_outcome, "affected_rows": affected,
        }),
    );
    if recycle {
        crate::java_pool_probe::exercise(&source, &observations, xid.as_str(), 1).await;
    }
    crate::java_pool_probe::wait_for_snapshot(&observations, "java-no-change-end-permitted").await;

    // 原 TM 保持全局结束权；本地响应不确定时，走原 XID 的回滚入口。
    let completion = if lose_ack {
        global.rollback().await
    } else {
        global.commit().await
    }
    .unwrap();
    let TransactionCompletion::Observed { outcome, .. } = completion else {
        panic!("original TC did not confirm global end: {completion:?}");
    };
    let expected = if lose_ack {
        GlobalStatus::Rollbacked
    } else {
        GlobalStatus::Committed
    };
    assert_eq!(outcome.status(), expected);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut statuses = Vec::new();
    loop {
        let status = tm.get_status(&xid).await.unwrap().status();
        statuses.push(status.to_java_v1());
        if status == GlobalStatus::Finished {
            break;
        }
        let expected_progress = if lose_ack {
            matches!(status, GlobalStatus::Rollbacking | GlobalStatus::Rollbacked)
        } else {
            matches!(
                status,
                GlobalStatus::Committing | GlobalStatus::AsyncCommitting | GlobalStatus::Committed
            )
        };
        assert!(
            expected_progress && tokio::time::Instant::now() < deadline,
            "{status:?}"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    fixture::save(
        "java-no-change-ended.json",
        &serde_json::json!({
            "xid": xid.as_str(), "observed_global_end": outcome.status().to_java_v1(), "status_queries": statuses,
        }),
    );
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
