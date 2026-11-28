//! 真实业务 DML/显式回滚计数；指标成功不替代数据库提交事实。
#![allow(clippy::unwrap_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_peer;
#[path = "support/stall_proxy.rs"]
#[allow(clippy::expect_used)]
mod stall_proxy;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use seata_db_mysql::{
    DbError, MySqlDataSource, MySqlPhaseOneOperation as Op, MySqlPhaseOneOutcome as Out,
};
use std::time::Duration;

async fn seed() -> (sqlx::MySqlConnection, MySqlDataSource) {
    let mut admin = owned_mysql::admin().await;
    sqlx::query("DROP TABLE IF EXISTS business_metrics")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE business_metrics(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("INSERT INTO business_metrics VALUES(1,10)")
        .execute(&mut admin)
        .await
        .unwrap();
    let source = MySqlDataSource::connect(
        owned_mysql::options(None)
            .operation_timeout(Duration::from_millis(1500))
            .unwrap(),
    )
    .await
    .unwrap();
    (admin, source)
}
async fn value(admin: &mut sqlx::MySqlConnection) -> i32 {
    sqlx::query_scalar("SELECT v FROM business_metrics WHERE id=1")
        .fetch_one(admin)
        .await
        .unwrap()
}
fn record(
    metrics: &seata_db_mysql::MySqlPhaseOneMetricsSampler,
    name: &str,
    mut data: serde_json::Value,
) {
    let stages: serde_json::Map<String, serde_json::Value> = Op::ALL
        .into_iter()
        .map(|op| {
            let counts = metrics.snapshot(op);
            let outcomes: serde_json::Map<String, serde_json::Value> = op
                .outcomes()
                .iter()
                .map(|outcome| (outcome.label().into(), counts.count(*outcome).into()))
                .collect();
            (
                op.label().into(),
                serde_json::json!({"active":counts.active,"outcomes":outcomes}),
            )
        })
        .collect();
    data.as_object_mut()
        .unwrap()
        .insert("stages".into(), serde_json::Value::Object(stages));
    let root = std::path::PathBuf::from(std::env::var_os("SEATA_BUSINESS_METRICS_OUT").unwrap());
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(name), serde_json::to_vec_pretty(&data).unwrap()).unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn completed_dml_is_not_committed_and_delegated_rollback_counts_once() {
    let (mut admin, source) = seed().await;
    let clone = source.clone();
    let metrics = source.phase_one_metrics_sampler();
    let mut tx = clone.begin().await.unwrap();
    let update =
        SqlRecognizerFactory::recognize("UPDATE business_metrics SET v=? WHERE id=1").unwrap();
    let never = tx.execute(&update, &[]);
    drop(never);
    assert_eq!(metrics.snapshot(Op::Execute), Default::default());
    assert_eq!(
        tx.execute(&update, &[FieldValue::Signed(11)])
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        metrics.snapshot(Op::Execute).count(Out::ExecutionCompleted),
        1,
        "business DML settlement missing"
    );
    let insert =
        SqlRecognizerFactory::recognize("INSERT INTO business_metrics(id,v) VALUES(2,20)").unwrap();
    assert_eq!(tx.execute(&insert, &[]).await.unwrap(), 1);
    let delete =
        SqlRecognizerFactory::recognize("DELETE FROM business_metrics WHERE id=2").unwrap();
    assert_eq!(tx.execute(&delete, &[]).await.unwrap(), 1);
    let zero =
        SqlRecognizerFactory::recognize("DELETE FROM business_metrics WHERE id=999").unwrap();
    assert_eq!(tx.execute(&zero, &[]).await.unwrap(), 0);
    assert_eq!(value(&mut admin).await, 10, "DML success is not commit");
    tx.rollback().await.unwrap();
    assert_eq!(value(&mut admin).await, 10);
    assert_eq!(
        metrics.snapshot(Op::Execute).count(Out::ExecutionCompleted),
        4
    );
    assert_eq!(
        clone
            .phase_one_metrics_sampler()
            .snapshot(Op::Rollback)
            .count(Out::RollbackConfirmed),
        1
    );
    // 未轮询的显式回滚 Future 只 Drop 原事务，不登记 API 尝试或取消。
    let unpolled_rollback = source.begin().await.unwrap().rollback();
    drop(unpolled_rollback);
    assert_eq!(metrics.snapshot(Op::Rollback).count(Out::Cancelled), 0);
    // 自动 Drop 仅释放原事务，不伪造一次显式 rollback API。
    drop(source.begin().await.unwrap());
    assert_eq!(
        metrics.snapshot(Op::Rollback).count(Out::RollbackConfirmed),
        1
    );
    source.close().await.unwrap();
    assert!(Op::ALL.iter().all(|op| metrics.snapshot(*op).active == 0));
    record(
        &metrics,
        "completed.json",
        serde_json::json!({"completed":metrics.snapshot(Op::Execute).count(Out::ExecutionCompleted),"rollback_confirmed":metrics.snapshot(Op::Rollback).count(Out::RollbackConfirmed),"visible_value":10,"active":0}),
    );
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn input_and_driver_errors_keep_the_original_transaction_uncommitted() {
    let (mut admin, source) = seed().await;
    let metrics = source.phase_one_metrics_sampler();
    let mut tx = source.begin().await.unwrap();
    let update =
        SqlRecognizerFactory::recognize("UPDATE business_metrics SET v=? WHERE id=1").unwrap();
    assert!(tx.execute(&update, &[]).await.is_err());
    assert_eq!(metrics.snapshot(Op::Execute).count(Out::ExecutionFailed), 1);
    assert_eq!(
        tx.execute(&update, &[FieldValue::Signed(12)])
            .await
            .unwrap(),
        1
    );
    let duplicate =
        SqlRecognizerFactory::recognize("INSERT INTO business_metrics(id,v) VALUES(1,50)").unwrap();
    assert!(matches!(
        tx.execute(&duplicate, &[]).await,
        Err(DbError::Driver {
            code: Some(1062),
            ..
        })
    ));
    assert!(matches!(
        tx.execute(&update, &[FieldValue::Signed(13)]).await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    assert_eq!(value(&mut admin).await, 10);
    assert_eq!(metrics.snapshot(Op::Execute).count(Out::ExecutionFailed), 3);
    assert_eq!(
        metrics.snapshot(Op::Execute).count(Out::ExecutionCompleted),
        1
    );
    assert_eq!(
        metrics.snapshot(Op::Rollback).count(Out::RollbackConfirmed),
        1
    );
    source.close().await.unwrap();
    record(
        &metrics,
        "errors.json",
        serde_json::json!({"failed":3,"completed":1,"rollback_confirmed":1,"visible_value":10}),
    );
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn blocked_dml_cancellation_and_deadline_are_not_successful_execution() {
    let (mut admin, source) = seed().await;
    let metrics = source.phase_one_metrics_sampler();
    sqlx::raw_sql("START TRANSACTION")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("UPDATE business_metrics SET v=10 WHERE id=1")
        .execute(&mut admin)
        .await
        .unwrap();
    let clone = source.clone();
    let task = tokio::spawn(async move {
        let mut tx = clone.begin().await.unwrap();
        let sql =
            SqlRecognizerFactory::recognize("UPDATE business_metrics SET v=20 WHERE id=1").unwrap();
        tx.execute(&sql, &[]).await
    });
    let mut observer = owned_mysql::admin().await;
    let limit = tokio::time::Instant::now() + Duration::from_secs(2);
    let waits = loop {
        let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='business_metrics'").fetch_one(&mut observer).await.unwrap();
        if waits > 0 {
            break waits;
        }
        assert!(
            tokio::time::Instant::now() < limit,
            "real business row wait missing"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(metrics.snapshot(Op::Execute).active, 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(metrics.snapshot(Op::Execute).active, 0);
    assert_eq!(metrics.snapshot(Op::Execute).count(Out::Cancelled), 1);
    let mut tx = source.begin().await.unwrap();
    let sql =
        SqlRecognizerFactory::recognize("UPDATE business_metrics SET v=30 WHERE id=1").unwrap();
    assert!(matches!(
        tx.execute(&sql, &[]).await,
        Err(DbError::Deadline { .. })
    ));
    assert!(matches!(tx.rollback().await, Err(DbError::Closed)));
    assert_eq!(
        metrics
            .snapshot(Op::Rollback)
            .count(Out::RollbackUnconfirmed),
        1
    );
    sqlx::raw_sql("ROLLBACK").execute(&mut admin).await.unwrap();
    assert_eq!(value(&mut admin).await, 10);
    assert_eq!(metrics.snapshot(Op::Execute).count(Out::ExecutionFailed), 1);
    assert_eq!(
        metrics.snapshot(Op::Execute).count(Out::ExecutionCompleted),
        0
    );
    assert_eq!(metrics.snapshot(Op::Execute).count(Out::Cancelled), 1);
    source.close().await.unwrap();
    record(
        &metrics,
        "waits.json",
        serde_json::json!({"real_row_waits":waits,"cancelled":1,"failed":1,"completed":0,"rollback_unconfirmed":1,"visible_value":10,"active":0}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn prepared_and_checked_delegates_each_count_one_explicit_rollback() {
    let (_admin, source) = seed().await;
    let metrics = source.phase_one_metrics_sampler();
    let manager = seata_db_mysql::MySqlDataSourceManager::new(
        source.clone(),
        MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = rm_peer::manager(&listener, Duration::from_secs(2)).await;
    let prepared = manager
        .prepare_transaction(
            source.begin().await.unwrap(),
            &rm.client(),
            "metrics:prepared",
            None,
        )
        .await
        .unwrap();
    prepared.rollback().await.unwrap();
    let checked = manager
        .prepare_transaction(
            source.begin().await.unwrap(),
            &rm.client(),
            "metrics:checked",
            None,
        )
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    checked.rollback().await.unwrap();
    assert_eq!(
        metrics.snapshot(Op::Rollback).count(Out::RollbackConfirmed),
        2,
        "explicit rollback settlement missing"
    );
    assert_eq!(metrics.snapshot(Op::Rollback).active, 0);
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
    record(
        &metrics,
        "delegates.json",
        serde_json::json!({"rollback_confirmed":2,"active":0}),
    );
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn rollback_response_withheld_keeps_cancellation_separate_from_unconfirmed() {
    let (mut admin, seed_source) = seed().await;
    seed_source.close().await.unwrap();
    let upstream = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    for cancelled in [true, false] {
        let proxy = stall_proxy::StallProxy::start(upstream).await;
        let source = MySqlDataSource::connect(
            owned_mysql::options(Some(proxy.port))
                .operation_timeout(Duration::from_secs(2))
                .unwrap(),
        )
        .await
        .unwrap();
        let metrics = source.phase_one_metrics_sampler();
        let mut tx = source.begin().await.unwrap();
        let sql =
            SqlRecognizerFactory::recognize("UPDATE business_metrics SET v=40 WHERE id=1").unwrap();
        tx.execute(&sql, &[]).await.unwrap();
        proxy.stall();
        let task = tokio::spawn(async move { tx.rollback().await });
        let limit = tokio::time::Instant::now() + Duration::from_secs(1);
        while proxy.held_bytes() == 0 {
            assert!(
                tokio::time::Instant::now() < limit,
                "rollback response bytes not held"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(metrics.snapshot(Op::Rollback).active, 1);
        // 独立连接已经能够锁定原行业务值，原回滚释放了真实行锁。
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT v FROM business_metrics WHERE id=1 FOR UPDATE")
                .fetch_one(&mut admin)
                .await
                .unwrap(),
            10
        );
        if cancelled {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            assert!(matches!(
                task.await.unwrap(),
                Err(DbError::Deadline {
                    operation: "rollback"
                })
            ));
        }
        assert_eq!(metrics.snapshot(Op::Rollback).active, 0);
        assert_eq!(
            metrics.snapshot(Op::Rollback).count(Out::RollbackConfirmed),
            0
        );
        assert_eq!(
            metrics.snapshot(Op::Rollback).count(Out::Cancelled),
            u64::from(cancelled)
        );
        assert_eq!(
            metrics
                .snapshot(Op::Rollback)
                .count(Out::RollbackUnconfirmed),
            u64::from(!cancelled)
        );
        source.close().await.unwrap();
        record(
            &metrics,
            if cancelled {
                "rollback-cancelled.json"
            } else {
                "rollback-unconfirmed.json"
            },
            serde_json::json!({"held_server_bytes":proxy.held_bytes(),"visible_value":10,"cancelled":u64::from(cancelled),"unconfirmed":u64::from(!cancelled),"confirmed":0,"active":0}),
        );
        proxy.shutdown().await;
    }
}
