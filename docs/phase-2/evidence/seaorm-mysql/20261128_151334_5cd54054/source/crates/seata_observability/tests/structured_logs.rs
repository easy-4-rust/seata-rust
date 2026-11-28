//! 固定模板导出、前置字段拒绝、有界队列与消费者关闭。
#![cfg(feature = "structured_logs")]
#![allow(clippy::unwrap_used)]
use seata_observability::{StructuredLogConfig, StructuredLogSubscriber};
use std::{
    fmt,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct ForbiddenDebug(Arc<AtomicUsize>);
impl fmt::Debug for ForbiddenDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fetch_add(1, Ordering::SeqCst);
        f.write_str("private-password-value")
    }
}
fn tm(xid: &str) {
    tracing::info!(target:"seata_tm::rpc",operation="Status",xid,request_id=-7,outcome="response","tm_rpc_outcome");
}
fn save(name: &str, bytes: &[u8]) {
    if let Some(folder) = std::env::var_os("SEATA_STRUCTURED_LOG_OUT") {
        let folder = std::path::PathBuf::from(folder);
        assert!(
            folder.is_absolute()
                && (folder.to_string_lossy().starts_with("D:")
                    || folder.to_string_lossy().starts_with("E:"))
        );
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(name), bytes).unwrap();
    }
}
#[test]
fn invalid_configuration_and_overflow_are_rejected() {
    for (capacity, bytes) in [
        (0, 512),
        (4097, 512),
        (1, 511),
        (1, 262145),
        (4096, 262144),
        (usize::MAX, usize::MAX),
    ] {
        assert!(StructuredLogConfig::new(capacity, bytes).is_err());
    }
    assert!(StructuredLogConfig::new(256, 262144).is_ok());
    assert!(StructuredLogConfig::new(4096, 512).is_ok());
}
#[tokio::test]
async fn unexpected_fields_are_rejected_before_debug_formatting() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(4, 2048).unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target:"seata_tm::rpc",operation="Status",xid="tc:1",request_id=1,outcome="response",password=?ForbiddenDebug(calls.clone()),"tm_rpc_outcome");
    });
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "unexpected field was formatted before schema rejection"
    );
    assert!(receiver.recv().await.is_none());
    assert_eq!(sampler.snapshot().dropped_invalid, 1);
}
#[tokio::test]
async fn seven_original_templates_preserve_fields_and_one_json_line_each() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(16, 4096).unwrap());
    let digest = "a".repeat(64);
    tracing::subscriber::with_default(subscriber, || {
        tm("tc:quote\"line\n");
        tracing::warn!(target:"seata_rm::rpc",operation="BranchReport",xid=?Some("tc:1"),branch_id=?Some(42_i64),resource_sha256=?Some(digest.as_str()),request_id=?Some(-17_i32),outcome="unknown","rm_rpc_outcome");
        tracing::info!(target:"seata_rm::callback",request_id=-17,connection_generation=2_u64,xid="tc:1",branch_id=42_i64,resource_sha256=digest.as_str(),phase="Rollback",branch_status="PhaseTwoRollbacked",result_code="Success","rm_callback_decision");
        tracing::info!(target:"seata_rm::callback_write",request_id=-17,connection_generation=2_u64,response_count=1_u64,outcome="local_written","rm_callback_response_write");
        tracing::info!(target:"seata_db_mysql::control",operation="rollback",requested_phase="rollback",xid="tc:1",branch_id=42_i64,resource_sha256=digest.as_str(),outcome="unknown_commit",resolution="not_observed",commit_attempted=true,"mysql_control_outcome");
        tracing::info!(target:"seata_db_mysql::undo",operation="rollback",xid="tc:1",branch_id=42_i64,resource_sha256=digest.as_str(),outcome="completed",observation="consumed",commit_attempted=true,"mysql_undo_outcome");
        tracing::warn!(target:"seata_db_mysql::undo",xid="tc:1",branch_id=42_i64,resource_digest=digest.as_str(),outcome="unknown","mysql_undo_checksum_rejected");
        tracing::info!(target:"unrelated_app",password="unrelated-secret","ignore application events");
        let span =
            tracing::info_span!(target:"seata_tm::rpc","private-span",password="span-secret");
        let _enter = span.enter();
    });
    let mut records = Vec::new();
    while let Some(bytes) = receiver.recv().await {
        assert_eq!(bytes.iter().filter(|b| **b == b'\n').count(), 1);
        assert!(bytes.len() <= 4096);
        records.push(serde_json::from_slice::<serde_json::Value>(&bytes).unwrap());
    }
    assert_eq!(records.len(), 7);
    assert_eq!(records[0]["fields"]["xid"], "tc:quote\"line\n");
    assert_eq!(records[0]["fields"]["request_id"], "-7");
    assert_eq!(records[1]["fields"]["request_id"], "Some(-17)");
    assert_eq!(records[4]["fields"]["outcome"], "unknown_commit");
    let stats = sampler.snapshot();
    assert_eq!(
        (stats.accepted, stats.received, stats.dropped_invalid),
        (7, 7, 0)
    );
    save(
        "seven-templates.json",
        &serde_json::to_vec_pretty(&records).unwrap(),
    );
}
#[tokio::test]
async fn full_queue_and_close_drop_new_records_then_drain_existing() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(2, 1024).unwrap());
    tracing::subscriber::with_default(subscriber.clone(), || {
        for _ in 0..6 {
            tm("tc:1");
        }
    });
    assert_eq!(
        (sampler.snapshot().accepted, sampler.snapshot().dropped_full),
        (2, 4)
    );
    receiver.close();
    tracing::subscriber::with_default(subscriber, || tm("tc:1"));
    let mut received = 0;
    while receiver.recv().await.is_some() {
        received += 1;
    }
    assert_eq!(received, 2);
    let stats = sampler.snapshot();
    assert_eq!(
        (
            stats.accepted,
            stats.received,
            stats.dropped_full,
            stats.dropped_closed
        ),
        (2, 2, 4, 1)
    );
}
#[tokio::test]
async fn oversized_fields_and_json_escaping_drop_whole_records() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(2, 512).unwrap());
    tracing::subscriber::with_default(subscriber, || {
        tm(&"x".repeat(1024));
        tm(&"\"".repeat(250));
    });
    assert!(
        receiver.recv().await.is_none(),
        "oversize record escaped JSON budget"
    );
    assert_eq!(sampler.snapshot().dropped_oversize, 2);
    assert_eq!(sampler.snapshot().accepted, 0);
}
#[tokio::test]
async fn invalid_values_and_messages_are_not_exported() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(2, 1024).unwrap());
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target:"seata_tm::rpc",operation="Status",xid="tc:1",request_id=1,outcome="private-error-text","tm_rpc_outcome");
        tracing::info!(target:"seata_tm::rpc",operation="Status",xid="tc:1",request_id=1,outcome="response","private-dynamic-message");
        tracing::info!(target:"seata_rm::rpc",operation="LockQuery",xid="tc:1",request_id=1,branch_id="None",resource_sha256="jdbc:mysql://private-user:private-password@localhost/db",outcome="response","rm_rpc_outcome");
    });
    assert!(
        receiver.recv().await.is_none(),
        "invalid field value reached JSON queue"
    );
    assert_eq!(sampler.snapshot().dropped_invalid, 3);
}
#[tokio::test]
async fn concurrent_full_queue_never_waits_for_consumer() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(4, 1024).unwrap());
    let barrier = Arc::new(std::sync::Barrier::new(9));
    let (done, done_rx) = std::sync::mpsc::channel();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let sub = subscriber.clone();
        let barrier = barrier.clone();
        let done = done.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            tracing::subscriber::with_default(sub, || {
                for _ in 0..256 {
                    tm("tc:1");
                }
            });
            done.send(()).unwrap();
        }));
    }
    barrier.wait();
    for _ in 0..8 {
        assert!(
            done_rx.recv_timeout(Duration::from_secs(5)).is_ok(),
            "log producer waited for full queue consumer"
        );
    }
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(
        (sampler.snapshot().accepted, sampler.snapshot().dropped_full),
        (4, 2044)
    );
    receiver.close();
    let mut n = 0;
    while receiver.recv().await.is_some() {
        n += 1;
    }
    assert_eq!(n, 4);
}
