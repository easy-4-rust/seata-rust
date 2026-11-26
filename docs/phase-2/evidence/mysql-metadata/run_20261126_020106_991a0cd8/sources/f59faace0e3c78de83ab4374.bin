//! 隔离容器中的签发节点与接管节点，私钥分别由测试控制面注入。
use super::{
    BasicConstraints, CertificateParams, Duration, Ed25519KeyPair, ExtendedKeyUsagePurpose,
    Identity, IsCa, Issuer, KeyPair, KeyUsagePurpose, LauncherProcessMonitor, LauncherStopSigner,
    LauncherStopTlsClient, LauncherStopTlsServer, LauncherStopVerifier, PrivateKeyDer,
    PrivatePkcs8KeyDer, RootCertStore, SystemRandom, TcpListener, TcpStream,
};
use rustls::pki_types::CertificateDer;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const SIGNER_NAME: &str = "signer.node.test";
const PORT: u16 = 4400;
const SCOPE: [u8; 32] = [0x76; 32];
const BUDGET: Duration = Duration::from_secs(8);

fn dir(variable: &str) -> PathBuf {
    std::env::var_os(variable).unwrap().into()
}

fn fixed<const N: usize>(path: impl AsRef<Path>) -> [u8; N] {
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
    let leaf = CertificateDer::from(fs::read(root.join("tls.der")).unwrap());
    let key = PrivatePkcs8KeyDer::from(fs::read(root.join("tls.key")).unwrap()).into();
    (roots, vec![leaf], key)
}

#[test]
#[ignore = "deployment runner provisions a fresh directory"]
fn provision() {
    let root = dir("SEATA_TM_NODE_PROVISION_DIR");
    let signer = root.join("signer");
    let recovery = root.join("recovery");
    fs::create_dir(&signer).unwrap();
    fs::create_dir(&recovery).unwrap();
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    let key = KeyPair::generate().unwrap();
    let ca = params.self_signed(&key).unwrap();
    let issuer = Issuer::new(params, key);
    let signer_tls = Identity::issue(&issuer, SIGNER_NAME, ExtendedKeyUsagePurpose::ServerAuth);
    let recovery_tls = Identity::issue(
        &issuer,
        "recovery.node.test",
        ExtendedKeyUsagePurpose::ClientAuth,
    );
    for (node, own, peer_pin) in [
        (&signer, &signer_tls, recovery_tls.pin()),
        (&recovery, &recovery_tls, signer_tls.pin()),
    ] {
        fs::write(node.join("ca.der"), ca.der()).unwrap();
        fs::write(node.join("tls.der"), &own.leaf).unwrap();
        fs::write(node.join("tls.key"), &own.pkcs8).unwrap();
        fs::write(node.join("peer.pin"), peer_pin).unwrap();
    }
    for generation in ["old", "new"] {
        let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let signing = LauncherStopSigner::from_pkcs8(key.as_ref(), SCOPE).unwrap();
        fs::write(signer.join(format!("{generation}.key")), key.as_ref()).unwrap();
        fs::write(
            recovery.join(format!("{generation}.pub")),
            signing.public_key(),
        )
        .unwrap();
    }
}

#[tokio::test]
#[ignore = "child of the isolated signing node"]
async fn old_owner() {
    let tm = super::node_peer::manager("127.0.0.1:1".parse().unwrap(), BUDGET);
    fs::write(
        dir("SEATA_TM_NODE_STATE_DIR").join("owner.bin"),
        tm.owner_instance_id(),
    )
    .unwrap();
    tm.announce_supervised_instance(std::io::stdout().lock())
        .unwrap();
    std::process::exit(0);
}

#[tokio::test]
#[ignore = "isolated signer node"]
async fn signer() {
    let root = dir("SEATA_TM_NODE_CREDENTIAL_DIR");
    let state = dir("SEATA_TM_NODE_STATE_DIR");
    let (roots, chain, key) = credentials(&root);
    let server =
        LauncherStopTlsServer::new(roots, chain, key, vec![fixed(root.join("peer.pin"))]).unwrap();
    let signer_for = |generation| {
        LauncherStopSigner::from_pkcs8(
            &fs::read(root.join(format!("{generation}.key"))).unwrap(),
            SCOPE,
        )
        .unwrap()
    };
    let old = signer_for("old");
    let new = signer_for("new");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", "--ignored", "node::old_owner"]);
    let mut monitor = LauncherProcessMonitor::spawn(&mut command).unwrap();
    let proof = tokio::time::timeout(BUDGET, async {
        loop {
            if let Some(proof) = monitor.try_confirm_exit().unwrap() {
                break proof;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(monitor.verifies(&proof, &fixed(state.join("owner.bin"))));
    let listener = TcpListener::bind(("0.0.0.0", PORT)).await.unwrap();
    fs::write(state.join("ready"), b"ready").unwrap();
    for (turn, signing) in [&old, &new, &old, &new].into_iter().enumerate() {
        let (stream, peer) = tokio::time::timeout(BUDGET, listener.accept())
            .await
            .unwrap()
            .unwrap();
        assert!(!peer.ip().is_loopback());
        if turn == 0 {
            println!(
                "NODE_LINK signer={} recovery={}",
                stream.local_addr().unwrap().ip(),
                peer.ip()
            );
        }
        server
            .respond(stream, &monitor, &proof, signing, BUDGET)
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "isolated recovery node"]
async fn recovery() {
    let root = dir("SEATA_TM_NODE_CREDENTIAL_DIR");
    let state = dir("SEATA_TM_NODE_STATE_DIR");
    // 接管节点的角色配置只有自己的 TLS 私钥及签发方停止公钥。
    assert!(!root.join("old.key").exists() && !root.join("new.key").exists());
    let (roots, chain, key) = credentials(&root);
    let client = LauncherStopTlsClient::new(
        SIGNER_NAME.try_into().unwrap(),
        roots,
        chain,
        key,
        vec![fixed(root.join("peer.pin"))],
    )
    .unwrap();
    let old_key = fixed(root.join("old.pub"));
    let new_key = fixed(root.join("new.pub"));
    let owner = fixed(state.join("owner.bin"));
    let tm = super::node_peer::manager("127.0.0.1:1".parse().unwrap(), BUDGET);
    let (verifier, mut rotation) = LauncherStopVerifier::new_rotatable(old_key, SCOPE);
    let challenge = verifier
        .challenge(&owner, tm.owner_instance_id(), BUDGET)
        .unwrap();
    let mut previous = None;
    for turn in 0..4 {
        if turn == 1 {
            rotation.replace(&[old_key, new_key]).unwrap();
        } else if turn == 2 {
            rotation.replace(&[new_key]).unwrap();
            assert!(!verifier.verifies(
                &challenge,
                &previous.unwrap(),
                &owner,
                tm.owner_instance_id()
            ));
        }
        let stream = tokio::time::timeout(BUDGET, TcpStream::connect((SIGNER_NAME, PORT)))
            .await
            .unwrap()
            .unwrap();
        assert!(!stream.peer_addr().unwrap().ip().is_loopback());
        let result = client.request(stream, &verifier, &challenge, BUDGET).await;
        if turn == 2 {
            assert!(
                matches!(result, Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied)
            );
        } else {
            let accepted = result.unwrap();
            assert!(verifier.verifies(&challenge, &accepted, &owner, tm.owner_instance_id()));
            if turn == 0 {
                previous = Some(accepted);
            }
        }
    }
    tm.shutdown().await.unwrap();
    println!("NODE_STOP_PROOF old_accepted=1 new_accepted=2 retired_rejected=1");
}
