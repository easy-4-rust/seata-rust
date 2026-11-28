//! 控制原 Future 的停止竞态测试；不冒充真实磁盘阻塞试验。
#![allow(clippy::unwrap_used)]
use super::preserve_io;
use crate::{StructuredLogReceiver, StructuredLogSampler};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

#[tokio::test]
async fn shutdown_closes_admission_but_keeps_original_inflight_future() {
    let (sender, rx) = mpsc::channel(2);
    sender
        .send(b"original-queued-record\n".to_vec())
        .await
        .unwrap();
    let mut receiver = StructuredLogReceiver {
        receiver: rx,
        sampler: StructuredLogSampler::default(),
    };
    let (stop, mut stopped) = oneshot::channel();
    let (entered, entry) = oneshot::channel();
    let (release, ready) = oneshot::channel();
    let task = tokio::spawn(async move {
        let mut stopping = false;
        let value = preserve_io(
            async move {
                entered.send(()).unwrap();
                ready.await.unwrap();
                Ok::<_, std::io::Error>(37)
            },
            &mut stopped,
            &mut receiver,
            &mut stopping,
        )
        .await
        .unwrap();
        assert!(stopping);
        assert_eq!(receiver.recv().await.unwrap(), b"original-queued-record\n");
        assert!(receiver.recv().await.is_none());
        value
    });
    entry.await.unwrap();
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), sender.closed())
        .await
        .unwrap_or_else(|_| {
            panic!("stop did not close admission while original I/O remained pending")
        });
    assert!(
        !task.is_finished(),
        "shutdown cancelled or replaced original I/O"
    );
    assert!(sender.try_send(b"late\n".to_vec()).is_err());
    release.send(()).unwrap();
    assert_eq!(task.await.unwrap(), 37);
}

#[tokio::test]
async fn production_worker_keeps_exact_original_record_bytes() {
    use crate::{log_file_progress::LogFileProgress, StructuredLogFileConfig};
    use std::{fs::OpenOptions, sync::Arc};
    let path = std::path::PathBuf::from("D:/Temp").join(format!(
        "seata-file-raw-{}-{}.ndjson",
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
    let (sender, rx) = mpsc::channel(2);
    let bytes = b"{\"original\":\"\\u4e2d\\n\",\"signed_id\":\"-2147483648\"}\n";
    sender.send(bytes.to_vec()).await.unwrap();
    drop(sender);
    let (_stop, stopped) = oneshot::channel();
    let report = super::run(
        StructuredLogReceiver {
            receiver: rx,
            sampler: StructuredLogSampler::default(),
        },
        file,
        StructuredLogFileConfig::new(4096, 64, Duration::from_secs(60)).unwrap(),
        stopped,
        Arc::new(LogFileProgress::default()),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        bytes,
        "worker re-encoded original record"
    );
    assert_eq!(report.synchronized_records, 1);
    assert_eq!(report.written_bytes, bytes.len() as u64);
    std::fs::remove_file(path).unwrap();
}
