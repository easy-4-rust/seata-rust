//! 真实 MySQL Undo 和关闭池的原控制结果进入正式有界 JSON 订阅器。
#![cfg(all(feature = "structured_log_file", feature = "mysql"))]
#![allow(clippy::unwrap_used)]
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{
    MySqlDataSource, MySqlDataSourceManager, MySqlOptions, UndoFenceObservation,
    UndoRollbackObservation,
};
use seata_observability::{StructuredLogConfig, StructuredLogSubscriber};
use seata_rm::{BranchIdentity, BranchPhase, BranchResolution, BranchResource};

#[tokio::test]
#[ignore = "requires owned real MySQL with original Undo schema"]
async fn original_mysql_undo_and_control_results_reach_json_file_without_credentials() {
    let (subscriber, receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(16, 4096).unwrap());
    #[cfg(feature = "structured_log_layer")]
    let dispatch = if std::env::var("SEATA_STRUCTURED_LOG_LAYER").as_deref() == Ok("1") {
        use tracing_subscriber::layer::SubscriberExt;
        tracing::Dispatch::new(
            tracing_subscriber::Registry::default().with(subscriber.into_layer()),
        )
    } else {
        tracing::Dispatch::new(subscriber)
    };
    #[cfg(not(feature = "structured_log_layer"))]
    let dispatch = tracing::Dispatch::new(subscriber);
    let _capture = tracing::dispatcher::set_default(&dispatch);
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
        std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap();
    let source = MySqlDataSource::connect(options).await.unwrap();
    let resource = source.resource_id().unwrap();
    assert_eq!(
        source.fence_undo_log("logs:fixture", 4242).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: true }
    );
    assert_eq!(
        source
            .rollback_undo_log("logs:fixture", 4242, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Fenced { inserted: false }
    );
    let manager =
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap();
    let identity = BranchIdentity::new(
        "logs:fixture".into(),
        4242,
        manager.key().clone(),
        Some("private-mysql-application-data".into()),
    )
    .unwrap();
    source.close().await.unwrap();
    assert_eq!(
        manager
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    let values=finish(receiver,&sampler,"mysql-native",serde_json::json!({"resource_id":resource,"xid":"logs:fixture","branch_id":4242,"fence_inserted":true,"rollback_read_existing_fence":true,"closed_pool_recovery":"Unknown","layer_mode":cfg!(feature = "structured_log_layer") && std::env::var("SEATA_STRUCTURED_LOG_LAYER").as_deref()==Ok("1")})).await;
    assert_eq!(values.len(), 3);
    assert_eq!(values[0]["fields"]["observation"], "fenced_inserted");
    assert_eq!(values[0]["fields"]["commit_attempted"], "true");
    assert_eq!(values[1]["fields"]["observation"], "fenced_existing");
    assert_eq!(values[1]["fields"]["commit_attempted"], "false");
    assert_eq!(values[2]["target"], "seata_db_mysql::control");
    assert_eq!(values[2]["fields"]["operation"], "bind_rollback");
    assert_eq!(values[2]["fields"]["outcome"], "before_commit_error");
    assert_eq!(values[2]["fields"]["commit_attempted"], "false");
    for event in values {
        let text = event.to_string();
        for secret in [
            "private-mysql-application-data",
            "seata-mysql-fixture-only",
            "jdbc:mysql",
            "INSERT INTO",
        ] {
            assert!(!text.contains(secret));
        }
    }
}

// 直接使用正式输出器；没有测试 helper 的文件写入或同步代替生产调用。
async fn finish(
    receiver: seata_observability::StructuredLogReceiver,
    sampler: &seata_observability::StructuredLogSampler,
    name: &str,
    proof: serde_json::Value,
) -> Vec<serde_json::Value> {
    use seata_observability::{
        StructuredLogFileConfig, StructuredLogFileShutdown, StructuredLogFileSink,
    };
    use std::{path::PathBuf, time::Duration};
    let folder = PathBuf::from(std::env::var_os("SEATA_STRUCTURED_LOG_OUT").unwrap());
    assert!(
        folder.is_absolute()
            && (folder.to_string_lossy().starts_with("D:")
                || folder.to_string_lossy().starts_with("E:"))
    );
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{name}.ndjson"));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let mut sink = StructuredLogFileSink::start(
        receiver,
        file,
        StructuredLogFileConfig::new(65536, 2, Duration::from_secs(60)).unwrap(),
    )
    .unwrap();
    let report = match sink.shutdown(Duration::from_secs(5)).await.unwrap() {
        StructuredLogFileShutdown::Complete(report) => report,
        other => panic!("native sink did not complete: {other:?}"),
    };
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(report.received_records, 3);
    assert_eq!(report.written_records, 3);
    assert_eq!(
        report.synchronized_records, 3,
        "MySQL records lack production sync confirmation"
    );
    assert_eq!(report.written_bytes, bytes.len() as u64);
    assert_eq!(report.sync_operations, 2);
    let values: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let s = sampler.snapshot();
    assert_eq!(s.accepted, 3);
    assert_eq!(s.received, 3);
    assert_eq!(
        s.dropped_invalid + s.dropped_closed + s.dropped_full + s.dropped_oversize,
        0
    );
    std::fs::write(folder.join(format!("{name}-proof.json")),serde_json::to_vec_pretty(&serde_json::json!({"source":proof,"stats":{"accepted":s.accepted,"received":s.received,"dropped_full":s.dropped_full,"dropped_closed":s.dropped_closed,"dropped_oversize":s.dropped_oversize,"dropped_invalid":s.dropped_invalid},"production_file_sink":true,"file_write_all_sync_all_and_readback_confirmed":true,"report":{"received_records":report.received_records,"written_records":report.written_records,"synchronized_records":report.synchronized_records,"written_bytes":report.written_bytes,"sync_operations":report.sync_operations}})).unwrap()).unwrap();
    values
}
