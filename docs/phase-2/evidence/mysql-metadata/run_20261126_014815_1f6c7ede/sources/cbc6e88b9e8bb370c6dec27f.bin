//! 停止证明由独立签发进程通过 TCP 传输，接管进程用预先配置的公钥核验。
#![allow(clippy::unwrap_used)]
use ring::signature::Ed25519KeyPair;
use seata_tm::{
    LauncherProcessMonitor, LauncherStopExchange, LauncherStopSigner, LauncherStopVerifier,
};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::Duration,
};
use tokio::net::{TcpListener, TcpStream};

const OWNER: [u8; 32] = [0x12; 32];
const SCOPE: [u8; 32] = [0x34; 32];
const REPLACEMENT: [u8; 32] = [0x56; 32];

#[test]
#[ignore = "only started by the remote signer fixture"]
fn stopped_worker_fixture() {
    // 固定报告只供可重复测试；真实工作进程由管理器报告其随机实例 ID。
    let mut output = std::io::stdout().lock();
    output.write_all(b"OWNER_INSTANCE=").unwrap();
    for byte in OWNER {
        write!(output, "{byte:02x}").unwrap();
    }
    output.write_all(b"\n").unwrap();
    output.flush().unwrap();
    std::process::exit(0);
}

#[tokio::test]
#[ignore = "only started by the recovering process fixture"]
async fn remote_signer_fixture() {
    let signer = {
        let mut key = Vec::new();
        std::io::stdin().lock().read_to_end(&mut key).unwrap();
        LauncherStopSigner::from_pkcs8(&key, SCOPE).unwrap()
    };
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--ignored", "--exact", "stopped_worker_fixture"]);
    let mut monitor = LauncherProcessMonitor::spawn(&mut command).unwrap();
    let proof = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if let Some(proof) = monitor.try_confirm_exit().unwrap() {
                break proof;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let endpoint = std::env::var("SEATA_STOP_EXCHANGE_FIXTURE_ENDPOINT").unwrap();
    let mut stream = TcpStream::connect(&endpoint).await.unwrap();
    LauncherStopExchange::respond(
        &mut stream,
        &monitor,
        &proof,
        &signer,
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    let mut unmatched = TcpStream::connect(&endpoint).await.unwrap();
    let denied = LauncherStopExchange::respond(
        &mut unmatched,
        &monitor,
        &proof,
        &signer,
        Duration::from_secs(2),
    )
    .await;
    assert!(matches!(denied, Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied));
}

#[tokio::test]
async fn verifies_stop_signature_received_from_separate_process() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let verifier;
    let mut child = {
        let key = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        verifier = LauncherStopVerifier::new(
            LauncherStopSigner::from_pkcs8(key.as_ref(), SCOPE)
                .unwrap()
                .public_key(),
            SCOPE,
        );
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "remote_signer_fixture"])
            .env(
                "SEATA_STOP_EXCHANGE_FIXTURE_ENDPOINT",
                listener.local_addr().unwrap().to_string(),
            )
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(key.as_ref()).unwrap();
        child
    };
    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(6), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let challenge = verifier
        .challenge(&OWNER, &REPLACEMENT, Duration::from_secs(4))
        .unwrap();
    let signature =
        LauncherStopExchange::request(&mut stream, &verifier, &challenge, Duration::from_secs(2))
            .await
            .unwrap();
    assert!(verifier.verifies(&challenge, &signature, &OWNER, &REPLACEMENT));
    assert!(!verifier.verifies(&challenge, &signature, &OWNER, &[0; 32]));
    let (mut unmatched, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let wrong_owner = verifier
        .challenge(&[0x78; 32], &REPLACEMENT, Duration::from_secs(3))
        .unwrap();
    let denied = LauncherStopExchange::request(
        &mut unmatched,
        &verifier,
        &wrong_owner,
        Duration::from_secs(2),
    )
    .await;
    assert!(matches!(denied, Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof));
    assert!(tokio::task::spawn_blocking(move || child.wait().unwrap())
        .await
        .unwrap()
        .success());
}

#[tokio::test]
async fn rejects_wrong_version_truncation_and_stalled_reply() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let verifier = LauncherStopVerifier::new([0; 32], SCOPE);
    let mut forged = [0; 68];
    forged[..4].copy_from_slice(b"SSR1");
    for (response, expected) in [
        (b"BAD!".as_slice(), std::io::ErrorKind::InvalidData),
        (b"".as_slice(), std::io::ErrorKind::UnexpectedEof),
        (forged.as_slice(), std::io::ErrorKind::PermissionDenied),
    ] {
        let (mut client, mut peer) = tokio::io::duplex(128);
        let challenge = verifier
            .challenge(&OWNER, &REPLACEMENT, Duration::from_secs(1))
            .unwrap();
        let exchange = LauncherStopExchange::request(
            &mut client,
            &verifier,
            &challenge,
            Duration::from_millis(100),
        );
        let (result, ()) = tokio::join!(exchange, async move {
            let mut request = [0; 68];
            peer.read_exact(&mut request).await.unwrap();
            peer.write_all(response).await.unwrap();
            peer.shutdown().await.unwrap();
        });
        assert_eq!(
            result.unwrap_err().kind(),
            expected,
            "response: {response:?}"
        );
    }
    let (mut client, _peer) = tokio::io::duplex(128);
    let challenge = verifier
        .challenge(&OWNER, &REPLACEMENT, Duration::from_secs(1))
        .unwrap();
    let result = LauncherStopExchange::request(
        &mut client,
        &verifier,
        &challenge,
        Duration::from_millis(20),
    )
    .await;
    assert!(matches!(result, Err(e) if e.kind() == std::io::ErrorKind::TimedOut));
}
