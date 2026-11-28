//! 调用方独占文件能力上的真实单文件输出及失败边界。
#![cfg(feature = "structured_log_file")]
#![allow(clippy::unwrap_used)]
use seata_observability::{
    StructuredLogConfig, StructuredLogFileConfig, StructuredLogFileError,
    StructuredLogFileShutdown, StructuredLogFileSink, StructuredLogFileStage,
    StructuredLogSubscriber,
};
use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> (Self, File) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = PathBuf::from("D:/Temp").join(format!(
            "seata-private-file-{}-{}.ndjson",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .read(true)
            .open(&path)
            .unwrap();
        (Self(path), file)
    }
    fn bytes(&self) -> Vec<u8> {
        std::fs::read(&self.0).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn config() -> StructuredLogFileConfig {
    StructuredLogFileConfig::new(65536, 64, Duration::from_secs(60)).unwrap()
}
fn channel() -> (
    StructuredLogSubscriber,
    seata_observability::StructuredLogReceiver,
    seata_observability::StructuredLogSampler,
) {
    StructuredLogSubscriber::channel(StructuredLogConfig::new(8, 4096).unwrap())
}
fn emit(subscriber: &StructuredLogSubscriber, id: i32) {
    tracing::subscriber::with_default(subscriber.clone(), || {
        tracing::info!(target:"seata_tm::rpc",operation="Status",xid="tc:file",request_id=id,outcome="response","tm_rpc_outcome");
    });
}

#[cfg(windows)]
#[tokio::test]
async fn readonly_empty_target_exercises_native_sync_all() {
    let (fixture, file) = Fixture::new();
    drop(file);
    let readonly = File::open(&fixture.0).unwrap();
    let (subscriber, receiver, _) = channel();
    drop(subscriber);
    let mut sink = StructuredLogFileSink::start(receiver, readonly, config()).unwrap();
    let error = match sink.wait().await {
        Err(error) => error,
        Ok(_) => panic!("read-only target must exercise native sync_all"),
    };
    match error {
        StructuredLogFileError::Io {
            stage,
            kind,
            os_error,
            progress,
            current_record_may_be_partial,
        } => {
            assert_eq!(stage, StructuredLogFileStage::Sync);
            assert_eq!(kind, std::io::ErrorKind::PermissionDenied);
            assert_eq!(os_error, Some(5));
            assert_eq!(progress.written_records, 0);
            assert_eq!(progress.synchronized_records, 0);
            assert_eq!(progress.sync_operations, 0);
            assert!(!current_record_may_be_partial);
        }
        other => panic!("unexpected native failure: {other:?}"),
    }
    assert!(fixture.bytes().is_empty());
}

#[tokio::test]
async fn shutdown_drains_original_worker_once_and_caches_result() {
    let (fixture, file) = Fixture::new();
    let (subscriber, receiver, sampler) = channel();
    emit(&subscriber, -17);
    emit(&subscriber, i32::MIN);
    let mut sink = StructuredLogFileSink::start(receiver, file, config()).unwrap();
    // 零预算允许已完成或 Pending；真实 Pending 的句柄保留另有受控 Future 测试。
    let first = sink.shutdown(Duration::ZERO).await.unwrap();
    assert!(matches!(
        first,
        StructuredLogFileShutdown::Pending | StructuredLogFileShutdown::Complete(_)
    ));
    let report = match sink.shutdown(Duration::from_secs(3)).await.unwrap() {
        StructuredLogFileShutdown::Complete(r) => r,
        other => panic!("not drained: {other:?}"),
    };
    assert_eq!(report.received_records, 2);
    assert_eq!(report.written_records, 2);
    assert_eq!(
        report.synchronized_records, 2,
        "shutdown prefix not synchronized"
    );
    assert_eq!(report.sync_operations, 1);
    let bytes = fixture.bytes();
    assert_eq!(bytes.len() as u64, report.written_bytes);
    assert_eq!(bytes.last(), Some(&b'\n'));
    let values: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        values.len(),
        2,
        "shutdown duplicated or lost original records"
    );
    assert_eq!(values[0]["fields"]["request_id"], "-17");
    assert_eq!(values[1]["fields"]["request_id"], i32::MIN.to_string());
    assert_eq!(sampler.snapshot().accepted, 2);
    assert_eq!(sampler.snapshot().received, 2);
    emit(&subscriber, 7);
    assert_eq!(sampler.snapshot().dropped_closed, 1);
    assert_eq!(sink.wait().await.unwrap(), report);
    assert_eq!(
        sink.shutdown(Duration::from_secs(3)).await.unwrap(),
        StructuredLogFileShutdown::Complete(report)
    );
    assert_eq!(fixture.bytes(), bytes);
}

async fn synchronized(sink: &StructuredLogFileSink, count: u64) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while sink.progress().synchronized_records < count {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("live synchronization missing"));
}
#[tokio::test]
async fn record_threshold_synchronizes_live_prefix_and_final_drain() {
    let (fixture, file) = Fixture::new();
    let (subscriber, receiver, _) = channel();
    emit(&subscriber, 1);
    emit(&subscriber, 2);
    let mut sink = StructuredLogFileSink::start(
        receiver,
        file,
        StructuredLogFileConfig::new(65536, 2, Duration::from_secs(60)).unwrap(),
    )
    .unwrap();
    synchronized(&sink, 2).await;
    assert_eq!(sink.progress().sync_operations, 1);
    assert_eq!(fixture.bytes().iter().filter(|&&b| b == b'\n').count(), 2);
    emit(&subscriber, 3);
    let report = match sink.shutdown(Duration::from_secs(3)).await.unwrap() {
        StructuredLogFileShutdown::Complete(r) => r,
        _ => panic!("final drain missing"),
    };
    assert_eq!(
        report.synchronized_records, 3,
        "final drain prefix not synchronized"
    );
    assert_eq!(report.sync_operations, 2);
    assert_eq!(fixture.bytes().iter().filter(|&&b| b == b'\n').count(), 3);
}
#[tokio::test]
async fn timer_synchronizes_dirty_live_prefix_without_idle_churn() {
    let (fixture, file) = Fixture::new();
    let (subscriber, receiver, _) = channel();
    emit(&subscriber, 1);
    let mut sink = StructuredLogFileSink::start(
        receiver,
        file,
        StructuredLogFileConfig::new(65536, 64, Duration::from_millis(20)).unwrap(),
    )
    .unwrap();
    synchronized(&sink, 1).await;
    assert_eq!(sink.progress().sync_operations, 1);
    assert!(!fixture.bytes().is_empty());
    tokio::time::sleep(Duration::from_millis(65)).await;
    assert_eq!(
        sink.progress().sync_operations,
        1,
        "idle timer repeated sync"
    );
    sink.request_shutdown();
    let report = sink.wait().await.unwrap();
    assert_eq!(report.synchronized_records, 1);
    assert_eq!(report.sync_operations, 2);
}
#[tokio::test]
async fn file_budget_rejects_next_record_and_synchronizes_existing_prefix() {
    let (subscriber, mut receiver, _) = channel();
    emit(&subscriber, 1);
    let length = receiver.recv().await.unwrap().len() as u64;
    let (fixture, file) = Fixture::new();
    let (subscriber, receiver, _) = channel();
    emit(&subscriber, 1);
    emit(&subscriber, 2);
    drop(subscriber);
    let mut sink = StructuredLogFileSink::start(
        receiver,
        file,
        StructuredLogFileConfig::new(length, 64, Duration::from_secs(60)).unwrap(),
    )
    .unwrap();
    let error = sink.wait().await.unwrap_err();
    let StructuredLogFileError::ByteLimit { progress } = error else {
        panic!("expected byte budget: {error:?}")
    };
    assert_eq!(progress.received_records, 2);
    assert_eq!(progress.written_records, 1);
    assert_eq!(
        progress.synchronized_records, 1,
        "budget error lost prefix sync"
    );
    assert_eq!(progress.sync_operations, 1);
    assert_eq!(fixture.bytes().len() as u64, length);
    assert_eq!(fixture.bytes().last(), Some(&b'\n'));
    assert_eq!(sink.wait().await.unwrap_err(), error);
}
#[tokio::test]
async fn first_record_over_budget_writes_nothing() {
    let (fixture, file) = Fixture::new();
    let (subscriber, receiver, _) = channel();
    emit(&subscriber, 1);
    let mut sink = StructuredLogFileSink::start(
        receiver,
        file,
        StructuredLogFileConfig::new(1, 64, Duration::from_secs(60)).unwrap(),
    )
    .unwrap();
    match sink.wait().await.unwrap_err() {
        StructuredLogFileError::ByteLimit { progress } => {
            assert_eq!(progress.received_records, 1);
            assert_eq!(progress.written_records, 0);
            assert_eq!(progress.sync_operations, 0);
        }
        other => panic!("unexpected: {other:?}"),
    }
    assert!(fixture.bytes().is_empty());
}
#[tokio::test]
async fn nonempty_or_displaced_files_are_rejected_without_modification() {
    use std::io::{Seek, SeekFrom, Write};
    for nonempty in [true, false] {
        let (fixture, mut file) = Fixture::new();
        if nonempty {
            file.write_all(b"caller-owned-original").unwrap();
            file.flush().unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
        } else {
            file.seek(SeekFrom::Start(9)).unwrap();
        }
        let before = fixture.bytes();
        let (subscriber, receiver, _) = channel();
        emit(&subscriber, 1);
        let mut sink = StructuredLogFileSink::start(receiver, file, config()).unwrap();
        assert_eq!(
            sink.wait().await,
            Err(StructuredLogFileError::InvalidTarget)
        );
        assert_eq!(fixture.bytes(), before);
    }
}
#[tokio::test]
async fn readonly_write_failure_is_typed_and_redacted() {
    let (fixture, file) = Fixture::new();
    drop(file);
    let readonly = File::open(&fixture.0).unwrap();
    let (subscriber, receiver, _) = channel();
    emit(&subscriber, 1);
    let mut sink = StructuredLogFileSink::start(receiver, readonly, config()).unwrap();
    let error = sink.wait().await.unwrap_err();
    match error {
        StructuredLogFileError::Io {
            stage,
            progress,
            current_record_may_be_partial,
            ..
        } => {
            assert!(matches!(
                stage,
                StructuredLogFileStage::Write | StructuredLogFileStage::Flush
            ));
            assert_eq!(progress.received_records, 1);
            assert_eq!(progress.written_records, 0);
            assert_eq!(progress.synchronized_records, 0);
            assert!(
                current_record_may_be_partial,
                "write failure lost unknown-tail warning"
            );
        }
        other => panic!("unexpected write result: {other:?}"),
    }
    let text = format!("{error:?} {error}");
    assert!(!text.contains("seata-private-file"));
    assert!(!text.contains("tc:file"));
    assert!(fixture.bytes().is_empty());
}
#[test]
fn configuration_and_missing_runtime_are_explicit_errors() {
    for (bytes, batch, interval) in [
        (0, 1, 10),
        (1_073_741_825, 1, 10),
        (1, 0, 10),
        (1, 65_537, 10),
        (1, 1, 9),
        (1, 1, 60_001),
    ] {
        assert_eq!(
            StructuredLogFileConfig::new(bytes, batch, Duration::from_millis(interval)),
            Err(StructuredLogFileError::InvalidConfiguration)
        );
    }
    let (_fixture, file) = Fixture::new();
    let (_, receiver, _) = channel();
    assert!(matches!(
        StructuredLogFileSink::start(receiver, file, config()),
        Err(StructuredLogFileError::RuntimeRequired)
    ));
}
#[tokio::test]
async fn invalid_shutdown_budget_does_not_request_stop() {
    let (_fixture, file) = Fixture::new();
    let (subscriber, receiver, sampler) = channel();
    let mut sink = StructuredLogFileSink::start(receiver, file, config()).unwrap();
    assert_eq!(
        sink.shutdown(Duration::from_secs(61)).await,
        Err(StructuredLogFileError::InvalidConfiguration)
    );
    emit(&subscriber, 1);
    assert_eq!(sampler.snapshot().accepted, 1);
    assert_eq!(sampler.snapshot().dropped_closed, 0);
    sink.request_shutdown();
    assert_eq!(sink.wait().await.unwrap().synchronized_records, 1);
}
