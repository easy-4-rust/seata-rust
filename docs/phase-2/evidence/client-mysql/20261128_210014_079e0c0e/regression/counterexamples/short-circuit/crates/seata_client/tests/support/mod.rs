#![allow(clippy::unwrap_used, dead_code)]
#[path = "../../../seata_transport/tests/support/session_peer.rs"]
pub mod peer;
use seata_rm::{FileBranchStore, JournalOptions};
use seata_transport::{ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub fn connection(endpoint: std::net::SocketAddr) -> ConnectionPoolConfig {
    ConnectionPoolConfig::new(
        endpoint,
        peer::config(8),
        1,
        8,
        ReconnectPolicy::new(
            Duration::from_millis(20),
            Duration::from_millis(80),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn identity() -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        "client-app-secret".into(),
        "client-group-secret".into(),
        Some("credential-secret".into()),
    )
    .unwrap()
}
pub fn options() -> JournalOptions {
    JournalOptions::new(8, 65536, 4096, 8).unwrap()
}
pub async fn journal() -> (PathBuf, FileBranchStore) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = PathBuf::from(format!(
        "D:/codex-builds/seata-client-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("branches.log");
    let store = FileBranchStore::open(path.clone(), options())
        .await
        .unwrap();
    (path, store)
}
