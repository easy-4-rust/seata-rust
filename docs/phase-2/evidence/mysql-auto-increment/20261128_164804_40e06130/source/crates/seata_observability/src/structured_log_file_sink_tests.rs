//! 运行时取消不等于成功关闭，不开放生产 abort 权限。
#![allow(clippy::unwrap_used)]
use crate::{
    StructuredLogConfig, StructuredLogFileConfig, StructuredLogFileError, StructuredLogFileSink,
    StructuredLogSubscriber,
};
use std::{fs::OpenOptions, time::Duration};

#[tokio::test]
async fn pending_and_cancelled_wait_keep_same_controlled_worker() {
    use crate::{
        log_file_progress::LogFileProgress, StructuredLogFileReport, StructuredLogFileShutdown,
    };
    use std::sync::Arc;
    use tokio::sync::oneshot;
    let (stop, stopped) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let expected = StructuredLogFileReport {
        received_records: 2,
        written_records: 2,
        written_bytes: 37,
        synchronized_records: 2,
        sync_operations: 1,
    };
    // 只控制 JoinHandle 生命周期，不宣称这里执行了磁盘 I/O。
    let worker = tokio::spawn(async move {
        stopped.await.unwrap();
        released.await.unwrap();
        Ok(expected)
    });
    let identity = worker.id();
    let mut sink = StructuredLogFileSink {
        worker: Some(worker),
        stop: Some(stop),
        progress: Arc::new(LogFileProgress::default()),
        terminal: None,
    };
    assert_eq!(
        sink.shutdown(Duration::ZERO).await.unwrap(),
        StructuredLogFileShutdown::Pending
    );
    assert_eq!(
        sink.worker.as_ref().unwrap().id(),
        identity,
        "Pending lost original worker"
    );
    assert!(tokio::time::timeout(Duration::from_millis(1), sink.wait())
        .await
        .is_err());
    assert_eq!(
        sink.worker.as_ref().unwrap().id(),
        identity,
        "cancelled wait lost original worker"
    );
    release.send(()).unwrap();
    assert_eq!(sink.wait().await.unwrap(), expected);
    assert_eq!(
        sink.shutdown(Duration::ZERO).await.unwrap(),
        StructuredLogFileShutdown::Complete(expected)
    );
}

#[tokio::test]
async fn cancelled_worker_reports_unknown_and_caches_it() {
    let path = std::path::PathBuf::from("D:/Temp").join(format!(
        "seata-file-cancel-{}-{}.ndjson",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let (_subscriber, receiver, _) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(2, 4096).unwrap());
    let mut sink = StructuredLogFileSink::start(
        receiver,
        file,
        StructuredLogFileConfig::new(4096, 64, Duration::from_secs(60)).unwrap(),
    )
    .unwrap();
    sink.worker.as_ref().unwrap().abort();
    let error = sink.wait().await.unwrap_err();
    assert!(
        matches!(error, StructuredLogFileError::WorkerOutcomeUnknown { .. }),
        "cancelled worker claimed known completion"
    );
    assert_eq!(sink.wait().await, Err(error));
    std::fs::remove_file(path).unwrap();
}
