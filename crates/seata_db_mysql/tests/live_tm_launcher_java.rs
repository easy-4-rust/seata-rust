//! 固定原 Java TC 的 TM 持久来源与结束回包丢失联试。
#![allow(clippy::unwrap_used)]
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_tm_at.rs"]
mod java_tm_at;
#[path = "support/java_tm_attested.rs"]
mod java_tm_attested;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod undo_fixture;

use seata_db_mysql::{MySqlDataSource, MySqlLauncherStore};
use seata_protocol::{v1::FrameLimits, GlobalStatus, PayloadLimits};
use seata_tm::{
    DurableTmError, LauncherOrigin, LauncherStore, TmError, TmOperation, TransactionConfig,
    TransactionManager, Xid,
};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
};
use sqlx::Row;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires pinned original Java TC, TM reply-loss proxy and owned MySQL"]
async fn original_java_tc_durable_tm_lost_commit_reply_never_replays() {
    durable_tm_lost_end_reply(false).await;
}

#[tokio::test]
#[ignore = "requires pinned original Java TC, TM reply-loss proxy and owned MySQL"]
async fn original_java_tc_durable_tm_lost_rollback_reply_never_replays() {
    durable_tm_lost_end_reply(true).await;
}

async fn durable_tm_lost_end_reply(rollback: bool) {
    let mut admin = owned_mysql::admin().await;
    apply_launcher_schema(&mut admin).await;
    let data_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(data_source.clone());
    let (pool, identity) = tm_connection();
    let endpoint = pool.endpoint();
    let tm = TransactionManager::builder(pool, identity.clone())
        .request_timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let config = launcher_config();
    let tx = tm
        .begin_durable(&config, "order-original-tc", &store)
        .await
        .unwrap();
    let xid = tx.xid().clone();
    let original_metrics = tm.metrics_sampler();
    observe_lost_end(rollback, &tm, tx, &mut admin, endpoint).await;
    tm.shutdown().await.unwrap();
    let observer = TransactionManager::builder(pool, identity)
        .request_timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let snapshot = store.load_snapshot(&xid).await.unwrap().unwrap();
    assert_ne!(snapshot.owner_instance_id(), observer.owner_instance_id());
    let outcome = snapshot
        .query_original_tc(&observer, "order-original-tc")
        .await
        .unwrap();
    let observed = outcome.status();
    assert!(if rollback {
        matches!(
            observed,
            GlobalStatus::Rollbacking | GlobalStatus::Rollbacked | GlobalStatus::Finished
        )
    } else {
        matches!(
            observed,
            GlobalStatus::Committing
                | GlobalStatus::AsyncCommitting
                | GlobalStatus::Committed
                | GlobalStatus::Finished
        )
    });
    let observer_metrics = observer.metrics_sampler();
    observer.shutdown().await.unwrap();
    let out = observation_dir();
    std::fs::write(
        out.join("tm-call-metrics.json"),
        serde_json::json!({
            "original": tm_metrics_value(&original_metrics),
            "observer": tm_metrics_value(&observer_metrics),
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(out.join("java-tm-durable-loss.json"),serde_json::json!({"xid":xid.as_str(),"unknown":true,"old_memory_recovery_denied":true,"send_started":1,"end_operation":if rollback {2} else {1},"observed_status":format!("{observed:?}"),"observed_status_code":observed.to_java_v1()}).to_string()).unwrap();
    data_source.close().await.unwrap();
}

fn tm_metrics_value(sampler: &seata_tm::TmMetricsSampler) -> serde_json::Value {
    let mut operations = serde_json::Map::new();
    for (operation, name) in [
        (TmOperation::Begin, "begin"),
        (TmOperation::Commit, "commit"),
        (TmOperation::Rollback, "rollback"),
        (TmOperation::Status, "status"),
    ] {
        let counts = sampler.snapshot(operation);
        operations.insert(
            name.into(),
            serde_json::json!({
                "active":counts.active, "observed":counts.observed,
                "tc_rejected":counts.tc_rejected, "unknown":counts.unknown,
                "not_sent":counts.not_sent, "local_rejected":counts.local_rejected,
                "cancelled":counts.cancelled, "protocol_errors":counts.protocol_errors,
            }),
        );
    }
    serde_json::Value::Object(operations)
}

async fn apply_launcher_schema(admin: &mut sqlx::MySqlConnection) {
    sqlx::raw_sql(include_str!(
        "../migrations/008_seata_rs_tm_launcher_v1.sql"
    ))
    .execute(&mut *admin)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/009_seata_rs_tm_launcher_owner_instance.sql"
    ))
    .execute(&mut *admin)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/010_seata_rs_tm_launcher_stop_scope.sql"
    ))
    .execute(&mut *admin)
    .await
    .unwrap();
}

fn tm_connection() -> (ConnectionPoolConfig, RegistrationIdentity) {
    let endpoint = std::env::var("SEATA_TM_ENDPOINT").unwrap().parse().unwrap();
    let identity = RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some("phase0-local-only".into()),
    )
    .unwrap();
    (pool_at(endpoint), identity)
}

fn pool_at(endpoint: std::net::SocketAddr) -> ConnectionPoolConfig {
    let connection = ConnectionConfig::new(
        FrameLimits::new(
            65536,
            32767,
            128,
            PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
        )
        .unwrap(),
        16,
        Duration::from_secs(3),
        Duration::from_secs(3),
    )
    .unwrap();
    ConnectionPoolConfig::new(
        endpoint,
        connection,
        1,
        16,
        ReconnectPolicy::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}

fn launcher_config() -> TransactionConfig {
    TransactionConfig::builder("tm-durable-original-tc".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap()
}

fn observation_dir() -> std::path::PathBuf {
    std::env::var("SEATA_UNDO_STORE_OUT").unwrap().into()
}

async fn observe_lost_end(
    rollback: bool,
    tm: &TransactionManager,
    tx: seata_tm::DurableGlobalTransaction<'_, MySqlLauncherStore>,
    admin: &mut sqlx::MySqlConnection,
    endpoint: std::net::SocketAddr,
) {
    let xid = tx.xid().clone();
    let error = if rollback {
        tx.rollback().await.unwrap_err()
    } else {
        tx.commit().await.unwrap_err()
    };
    let DurableTmError::Tm(error @ TmError::UnknownOutcome { .. }) = error else {
        panic!("original TC end response was lost, outcome must remain unknown");
    };
    assert!(tm.recover_unknown(&error).is_err());
    let row = sqlx::query("SELECT xid,tc_endpoint,application_id,service_group,business_intent,end_operation,send_started,owner_epoch FROM seata_rs_tm_launcher_v1")
        .fetch_one(admin).await.unwrap();
    assert_eq!(row.get::<Vec<u8>, _>("xid"), xid.as_str().as_bytes());
    assert_eq!(
        row.get::<Vec<u8>, _>("tc_endpoint"),
        endpoint.to_string().as_bytes()
    );
    assert_eq!(row.get::<Vec<u8>, _>("application_id"), b"phase0-auth");
    assert_eq!(row.get::<Vec<u8>, _>("service_group"), b"default_tx_group");
    assert_eq!(
        row.get::<Vec<u8>, _>("business_intent"),
        b"order-original-tc"
    );
    assert_eq!(
        (
            row.get::<u8, _>("end_operation"),
            row.get::<u8, _>("send_started"),
            row.get::<u64, _>("owner_epoch")
        ),
        (if rollback { 2 } else { 1 }, 1, 1)
    );
}

struct ExitAfterIntent(MySqlLauncherStore);

impl LauncherStore for ExitAfterIntent {
    async fn record_begin(
        &self,
        origin: &LauncherOrigin,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.record_begin(origin).await
    }

    async fn record_end_intent(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.record_end_intent(origin, operation).await?;
        std::process::exit(0);
    }

    async fn mark_send_started(
        &self,
        _origin: &LauncherOrigin,
        _operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        panic!("the old process must exit before the send marker");
    }
}

#[tokio::test]
#[ignore = "child of the pinned Java TC recovery scenarios"]
async fn original_java_tc_launcher_intent_child() {
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = ExitAfterIntent(MySqlLauncherStore::new(source));
    let (pool, identity) = tm_connection();
    let mut builder = TransactionManager::builder(pool, identity);
    if std::env::var_os("SEATA_TM_ATTESTATION_CREDENTIALS").is_some() {
        builder = builder.launcher_stop_scope(java_tm_attested::SCOPE);
    }
    let tm = builder.build().unwrap();
    tm.announce_supervised_instance(std::io::stdout().lock())
        .unwrap();
    std::fs::write(
        observation_dir().join("old-tm-pid"),
        std::process::id().to_string(),
    )
    .unwrap();
    let tx = tm
        .begin_durable(&launcher_config(), "order-java-takeover", &store)
        .await
        .unwrap();
    #[cfg(feature = "experimental_changed_commit")]
    if std::env::var_os("SEATA_JAVA_TM_RECOVERY_AT").is_some() {
        let at = java_tm_at::JavaTmAt::start("old-tm-at.log").await;
        at.commit_phase_one(tx.xid().as_str()).await;
        at.shutdown().await;
    }
    if std::env::var_os("SEATA_JAVA_TM_RECOVERY_ROLLBACK").is_some() {
        let _ = tx.rollback().await;
    } else {
        let _ = tx.commit().await;
    }
    panic!("intent persistence must be confirmed before the old process exits");
}

#[tokio::test]
#[ignore = "requires pinned original Java TC, raw TM observer and owned MySQL"]
async fn original_java_tc_supervised_tm_recovers_commit() {
    supervised_java_recovery(false, false).await;
}

#[tokio::test]
#[ignore = "requires pinned original Java TC, TM reply-loss observer and owned MySQL"]
async fn original_java_tc_supervised_tm_recovers_rollback_with_reply_loss() {
    supervised_java_recovery(true, false).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires original Java TC, a real AT branch and owned MySQL"]
async fn original_java_tc_supervised_tm_commits_at_branch() {
    supervised_java_recovery(false, true).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires original Java TC, a real AT branch and owned MySQL"]
async fn original_java_tc_supervised_tm_rolls_back_at_branch_with_reply_loss() {
    supervised_java_recovery(true, true).await;
}

async fn supervised_java_recovery(rollback: bool, with_at: bool) {
    #[cfg(feature = "experimental_changed_commit")]
    let mut admin = if with_at {
        java_tm_at::reset().await
    } else {
        owned_mysql::admin().await
    };
    #[cfg(not(feature = "experimental_changed_commit"))]
    let mut admin = owned_mysql::admin().await;
    apply_launcher_schema(&mut admin).await;
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(source.clone());
    let remote = std::env::var_os("SEATA_ATTESTED_TEST_SIGNER_ENDPOINT").is_some();
    let local_proof = if remote {
        let out = observation_dir();
        std::fs::write(out.join("attested-schema-ready"), b"ready").unwrap();
        tokio::time::timeout(Duration::from_secs(30), async {
            while !out.join("attested-signer-ready").is_file() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        None
    } else {
        Some(java_tm_attested::supervise_old_owner(rollback, with_at).await)
    };
    let old_pid = std::fs::read_to_string(observation_dir().join("old-tm-pid"))
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let xid: Vec<u8> = sqlx::query_scalar("SELECT xid FROM seata_rs_tm_launcher_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    let xid = Xid::parse(String::from_utf8(xid).unwrap()).unwrap();
    let snapshot = store.load_snapshot(&xid).await.unwrap().unwrap();
    let operation = if rollback {
        TmOperation::Rollback
    } else {
        TmOperation::Commit
    };
    assert_eq!(snapshot.owner_epoch(), 1);
    assert_eq!(snapshot.end_operation(), Some(operation));
    assert!(!snapshot.send_started());
    assert_eq!(
        snapshot.owner_stop_scope(),
        remote.then_some(&java_tm_attested::SCOPE)
    );
    #[cfg(feature = "experimental_changed_commit")]
    let at_branch = if with_at {
        let at = java_tm_at::JavaTmAt::start("new-tm-at.log").await;
        let branch = at.branch_before_end(xid.as_str(), &mut admin).await;
        Some((at, branch))
    } else {
        None
    };
    let (pool, identity) = tm_connection();
    let (tm, authority) = TransactionManager::builder(pool, identity)
        .build_with_recovered_end_authority()
        .unwrap();
    let claim = if let Some((monitor, proof)) = &local_proof {
        store
            .transfer_stopped_owner(&snapshot, monitor, proof, &tm, "order-java-takeover")
            .await
            .unwrap()
    } else {
        java_tm_attested::claim(&store, &snapshot, &tm).await
    };
    assert_eq!(claim.epoch(), 2);
    let result = claim.finish(&authority, &store).await;
    let observed = if rollback {
        let Err(DurableTmError::Tm(error @ TmError::UnknownOutcome { .. })) = result else {
            panic!("recovered Rollback reply loss must remain unknown: {result:?}");
        };
        assert!(tm.recover_unknown(&error).is_err());
        None
    } else {
        let status = result.unwrap().status();
        assert_eq!(status, GlobalStatus::Committed);
        Some(format!("{status:?}"))
    };
    let sent = store.load_snapshot(&xid).await.unwrap().unwrap();
    assert_eq!(sent.owner_instance_id(), tm.owner_instance_id());
    assert_eq!(sent.end_operation(), Some(operation));
    assert!(sent.send_started());
    if let Some((monitor, proof)) = &local_proof {
        assert!(store
            .transfer_stopped_owner(&sent, monitor, proof, &tm, "order-java-takeover")
            .await
            .is_err());
    }
    // 独立数据库查询确认 TC 最终清理原会话，不增加 TC 状态查询或推断丢失回包。
    tokio::time::timeout(Duration::from_secs(16), async {
        loop {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM global_table WHERE xid=?")
                .bind(xid.as_str())
                .fetch_one(&mut admin)
                .await
                .unwrap();
            if count == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    #[cfg(feature = "experimental_changed_commit")]
    let branch_id = if let Some((at, branch)) = at_branch {
        at.verify_terminal(xid.as_str(), branch, rollback, &mut admin)
            .await;
        at.shutdown().await;
        Some(branch)
    } else {
        None
    };
    #[cfg(not(feature = "experimental_changed_commit"))]
    let branch_id: Option<i64> = None;
    tm.shutdown().await.unwrap();
    source.close().await.unwrap();
    std::fs::write(
        observation_dir().join("java-tm-recovered.json"),
        serde_json::json!({
            "xid": xid.as_str(), "old_pid": old_pid, "new_pid": std::process::id(),
            "unknown": rollback, "old_memory_recovery_denied": rollback,
            "observed_end_status": observed,
            "branch_id": branch_id,
            "remote_stop_proof": remote,
            "origin_stop_scope_bound": snapshot.owner_stop_scope().is_some(),
        })
        .to_string(),
    )
    .unwrap();
}
