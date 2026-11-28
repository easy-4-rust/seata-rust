//! 签发容器监督真实旧 TM，接管方经双向 TLS 获取证明。
use super::{observation_dir, tm_connection};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    RootCertStore,
};
use seata_db_mysql::{MySqlLauncherSnapshot, MySqlLauncherStore, MySqlTransferredLauncher};
use seata_tm::{
    LauncherProcessMonitor, LauncherStopSigner, LauncherStopTlsClient, LauncherStopTlsServer,
    LauncherStopVerifier, StoppedLauncherOwner, TransactionManager,
};
use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tokio::net::{TcpListener, TcpStream};

const HOST: &str = "signer.node.test";
pub(super) const SCOPE: [u8; 32] = [0x76; 32];
const BUDGET: Duration = Duration::from_secs(8);

fn configuration() -> PathBuf {
    std::env::var_os("SEATA_TM_ATTESTATION_CREDENTIALS")
        .unwrap()
        .into()
}

fn read_fixed<const N: usize>(path: &Path) -> [u8; N] {
    fs::read(path).unwrap().try_into().unwrap()
}

fn credentials(
    root: &Path,
) -> (
    RootCertStore,
    Vec<CertificateDer<'static>>,
    PrivateKeyDer<'static>,
) {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(fs::read(root.join("ca.der")).unwrap()))
        .unwrap();
    let chain = vec![CertificateDer::from(
        fs::read(root.join("tls.der")).unwrap(),
    )];
    let key = PrivatePkcs8KeyDer::from(fs::read(root.join("tls.key")).unwrap()).into();
    (roots, chain, key)
}

pub async fn supervise_old_owner(
    rollback: bool,
    with_at: bool,
) -> (LauncherProcessMonitor, StoppedLauncherOwner) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([
        "original_java_tc_launcher_intent_child",
        "--ignored",
        "--exact",
    ]);
    if rollback {
        command.env("SEATA_JAVA_TM_RECOVERY_ROLLBACK", "1");
    }
    if with_at {
        command.env("SEATA_JAVA_TM_RECOVERY_AT", "1");
    }
    let mut monitor = LauncherProcessMonitor::spawn(&mut command).unwrap();
    let proof = tokio::time::timeout(Duration::from_secs(18), async {
        loop {
            if let Some(proof) = monitor.try_confirm_exit().unwrap() {
                break proof;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    (monitor, proof)
}

async fn relay(port: u16) -> tokio::task::JoinHandle<()> {
    // 两个 TM 使用相同数值端点。容器回环端口经专用夹具中继访问宿主原端点。
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    tokio::spawn(async move {
        while let Ok((mut client, _)) = listener.accept().await {
            tokio::spawn(async move {
                if let Ok(mut upstream) = TcpStream::connect(("host.docker.internal", port)).await {
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
                }
            });
        }
    })
}

#[tokio::test]
#[ignore = "owned Docker signing node for fixed original TC AT recovery"]
async fn signer_node() {
    let root = configuration();
    let (roots, chain, key) = credentials(&root);
    let server =
        LauncherStopTlsServer::new(roots, chain, key, vec![read_fixed(&root.join("peer.pin"))])
            .unwrap();
    let signer =
        LauncherStopSigner::from_pkcs8(&fs::read(root.join("new.key")).unwrap(), SCOPE).unwrap();
    let (pool, _) = tm_connection();
    let rm: SocketAddr = std::env::var("SEATA_RM_ENDPOINT").unwrap().parse().unwrap();
    let mysql = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let relays = [
        relay(pool.endpoint().port()).await,
        relay(rm.port()).await,
        relay(mysql).await,
    ];
    let rollback = std::env::var_os("SEATA_ATTESTED_TEST_ROLLBACK").is_some();
    let (monitor, proof) = supervise_old_owner(rollback, true).await;
    let listener = TcpListener::bind(("0.0.0.0", 4400)).await.unwrap();
    // 仅在操作系统确认旧 TM 退出、TLS 监听已启动后公布就绪状态。
    fs::write(observation_dir().join("attested-signer-ready"), b"ready").unwrap();
    let (stream, peer) = tokio::time::timeout(Duration::from_secs(20), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert!(!peer.ip().is_loopback());
    server
        .respond(stream, &monitor, &proof, &signer, BUDGET)
        .await
        .unwrap();
    println!("ATTESTED_SIGNER proof_sent=1 peer={}", peer.ip());
    for task in relays {
        task.abort();
    }
}

pub async fn claim<'a>(
    store: &MySqlLauncherStore,
    snapshot: &'a MySqlLauncherSnapshot,
    tm: &'a TransactionManager,
) -> MySqlTransferredLauncher<'a> {
    let root = configuration();
    assert!(!root.join("new.key").exists() && !root.join("old.key").exists());
    let (roots, chain, key) = credentials(&root);
    let client = LauncherStopTlsClient::new(
        HOST.try_into().unwrap(),
        roots,
        chain,
        key,
        vec![read_fixed(&root.join("peer.pin"))],
    )
    .unwrap();
    let verifier = LauncherStopVerifier::new(read_fixed(&root.join("new.pub")), SCOPE);
    let challenge = verifier
        .challenge(snapshot.owner_instance_id(), tm.owner_instance_id(), BUDGET)
        .unwrap();
    let address: SocketAddr = std::env::var("SEATA_ATTESTED_TEST_SIGNER_ENDPOINT")
        .unwrap()
        .parse()
        .unwrap();
    let stream = TcpStream::connect(address).await.unwrap();
    let signature = client
        .request(stream, &verifier, &challenge, BUDGET)
        .await
        .unwrap();
    store
        .transfer_attested_owner(
            snapshot,
            &verifier,
            &challenge,
            &signature,
            tm,
            "order-java-takeover",
        )
        .await
        .unwrap()
}
