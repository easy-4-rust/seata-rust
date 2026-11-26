#![allow(clippy::unwrap_used, dead_code)]
#[path = "../../../seata_transport/tests/support/session_peer.rs"]
pub mod peer;
use seata_protocol::protocol::RegisterRMResponse;
use seata_protocol::{BranchType, Message};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchResolution, BranchResource, FileBranchStore, JournalOptions,
    ResourceCatalog, ResourceManager, RmError,
};
use seata_transport::{ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity, ResourceKey};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
pub struct Resource {
    pub key: ResourceKey,
    pub actions: AtomicU64,
    pub stall: bool,
    pub entered: tokio::sync::Notify,
    pub pause: bool,
    pub release: tokio::sync::Notify,
    pub panic_on_drop: bool,
}
struct PanicOnDrop;
impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("fixture adapter cleanup panicked");
    }
}
impl BranchResource for Resource {
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn apply<'a>(
        &'a self,
        _identity: &'a BranchIdentity,
        _phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        self.actions.fetch_add(1, Ordering::SeqCst);
        if self.panic_on_drop {
            return Box::pin(async move {
                let _guard = PanicOnDrop;
                self.entered.notify_one();
                std::future::pending::<()>().await;
                Ok(BranchResolution::Applied)
            });
        }
        self.entered.notify_one();
        if self.pause {
            Box::pin(async move {
                self.release.notified().await;
                Ok(BranchResolution::Applied)
            })
        } else if self.stall {
            Box::pin(std::future::pending())
        } else {
            Box::pin(async { Ok(BranchResolution::Applied) })
        }
    }
    fn recover<'a>(
        &'a self,
        _identity: &'a BranchIdentity,
        _phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(async { Ok(BranchResolution::Unknown) })
    }
}
pub fn settings(endpoint: std::net::SocketAddr) -> ConnectionPoolConfig {
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
pub fn registration() -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        "rm-app".into(),
        "rm-group".into(),
        Some("rm-public-fixture".into()),
    )
    .unwrap()
}
pub fn resource() -> Arc<Resource> {
    Arc::new(Resource {
        key: ResourceKey::new(BranchType::At, "mysql://exact-resource".into()).unwrap(),
        actions: AtomicU64::new(0),
        stall: false,
        entered: tokio::sync::Notify::new(),
        pause: false,
        release: tokio::sync::Notify::new(),
        panic_on_drop: false,
    })
}
pub async fn journal() -> FileBranchStore {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let folder = std::path::PathBuf::from(format!(
        "D:/codex-builds/rm-native-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&folder).unwrap();
    FileBranchStore::open(
        folder.join("branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap()
}
pub async fn manager(
    listener: &TcpListener,
    timeout: Duration,
) -> (ResourceManager, Arc<Resource>) {
    let resource = resource();
    let catalog = ResourceCatalog::new(4, 4096).unwrap();
    catalog.register(resource.clone()).unwrap();
    let manager = ResourceManager::builder(
        settings(listener.local_addr().unwrap()),
        registration(),
        catalog,
        journal().await,
    )
    .request_timeout(timeout)
    .build()
    .unwrap();
    (manager, resource)
}
pub async fn registered(listener: &TcpListener) -> TcpStream {
    let (mut socket, _) = listener.accept().await.unwrap();
    let request = peer::read(&mut socket).await;
    match request.body {
        Some(Message::RegisterRMRequest(value)) => {
            assert_eq!(value.application_id.as_deref(), Some("rm-app"));
            assert_eq!(value.transaction_service_group, "rm-group");
            assert_eq!(value.extra_data.as_deref(), Some("rm-public-fixture"));
            assert_eq!(
                value.resource_ids.as_deref(),
                Some("mysql://exact-resource")
            );
        }
        other => panic!("{other:?}"),
    }
    peer::write(
        &mut socket,
        peer::reply(
            request.id,
            Message::RegisterRMResponse(RegisterRMResponse {
                identified: true,
                wire_version: None,
            }),
        ),
    )
    .await;
    socket
}
