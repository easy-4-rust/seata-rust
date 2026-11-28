//! 真实 TLS/MySQL 下，持久来源必须拒绝 TC 信任降级。
#![cfg(feature = "transport_tls")]
#![allow(clippy::unwrap_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "../../seata_transport/tests/support/tls_peer.rs"]
mod tls_peer;
use seata_db_mysql::{MySqlDataSource, MySqlLauncherStore};
use seata_protocol::{GlobalStatus, Message};
use seata_tm::{TmError, TransactionConfig, TransactionManager};
use seata_transport::RegistrationIdentity;
use sqlx::Executor;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    net::TcpListener,
};
#[path = "../../seata_tm/tests/support/tm_peer.rs"]
mod tm_fixture;
use seata_tm::{LauncherOrigin, LauncherProcessMonitor, LauncherStore, TmOperation};
use tm_fixture::peer;

fn save(name: &str, value: serde_json::Value) {
    if let Some(folder) = std::env::var_os("SEATA_TM_TRUST_OUT") {
        let folder = std::path::PathBuf::from(folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join(name),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
}

async fn optional_frame(
    stream: &mut (impl AsyncRead + Unpin),
) -> Option<seata_protocol::protocol::RpcMessage> {
    let mut header = [0; 16];
    match stream.read_exact(&mut header[..1]).await {
        Ok(_) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset
            ) =>
        {
            return None
        }
        Err(error) => panic!("fixture read failed: {error}"),
    }
    stream.read_exact(&mut header[1..]).await.unwrap();
    let size = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
    assert!((16..=65536).contains(&size));
    let mut bytes = header.to_vec();
    bytes.resize(size, 0);
    stream.read_exact(&mut bytes[16..]).await.unwrap();
    Some(
        seata_protocol::v1::ProtocolDecoderV1::new(tls_peer::limits())
            .decode(&bytes)
            .unwrap(),
    )
}

fn identity() -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        "tm-trust".into(),
        "tm-trust-group".into(),
        None,
    )
    .unwrap()
}

async fn serve(stream: &mut (impl AsyncRead + AsyncWrite + Unpin), queries: &AtomicU64) {
    while let Some(request) = optional_frame(stream).await {
        let body = match request.body.unwrap() {
            Message::RegisterTMRequest(_) => {
                Message::RegisterTMResponse(seata_protocol::protocol::RegisterTMResponse {
                    identified: true,
                    wire_version: None,
                })
            }
            Message::GlobalBeginRequest(_) => peer::begin_result("tc:launcher-trust"),
            Message::GlobalStatusRequest(_) => {
                queries.fetch_add(1, Ordering::SeqCst);
                Message::GlobalStatusResponse(seata_protocol::transaction::GlobalStatusResponse {
                    response: peer::success(),
                    global_status: GlobalStatus::Committed,
                })
            }
            other => panic!("unexpected original frame: {other:?}"),
        };
        tls_peer::write(stream, peer::reply(request.id, body)).await;
    }
}

#[tokio::test]
#[ignore = "requires owned MySQL and native mTLS fixture"]
async fn persisted_tls_origin_rejects_plaintext_replacement_before_status_request() {
    let mut admin = owned_mysql::admin().await;
    admin
        .execute("DROP TABLE IF EXISTS seata_rs_tm_launcher_v1")
        .await
        .unwrap();
    for migration in [
        include_str!("../migrations/008_seata_rs_tm_launcher_v1.sql"),
        include_str!("../migrations/009_seata_rs_tm_launcher_owner_instance.sql"),
        include_str!("../migrations/010_seata_rs_tm_launcher_stop_scope.sql"),
        include_str!("../migrations/014_seata_rs_tm_transport_binding.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(source.clone());
    let pki = tls_peer::Pki::new();
    let security = pki.client("seata.example.test");
    let acceptor = pki.acceptor();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let queries = Arc::new(AtomicU64::new(0));
    let observed = queries.clone();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut tls = acceptor.accept(socket).await.unwrap();
        serve(&mut tls, &observed).await;
        drop(tls);
        // 同一个受控端点随后提供明文对端，模拟端口复用后的降级。
        let (mut socket, _) = listener.accept().await.unwrap();
        serve(&mut socket, &observed).await;
    });
    let original = TransactionManager::builder(tls_peer::pool(address), identity())
        .transport_tls(security)
        .build()
        .unwrap();
    let config = TransactionConfig::builder("trust-source".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let durable = original
        .begin_durable(&config, "trust-business", &store)
        .await
        .unwrap();
    let xid = durable.xid().clone();
    drop(durable);
    original.shutdown().await.unwrap();
    let snapshot = store.load_snapshot(&xid).await.unwrap().unwrap();
    let replacement = TransactionManager::builder(tls_peer::pool(address), identity())
        .request_timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while replacement.connection_snapshot().ready() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let result = snapshot
        .query_original_tc(&replacement, "trust-business")
        .await;
    assert!(
        matches!(result, Err(TmError::IllegalState { .. })),
        "TLS source cannot trust a plaintext replacement: {result:?}"
    );
    assert_eq!(queries.load(Ordering::SeqCst), 0);
    let version: u16 = sqlx::query_scalar("SELECT schema_version FROM seata_rs_tm_launcher_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    save(
        "downgrade.json",
        serde_json::json!({"source_schema_version":version,"stored_binding":snapshot.peer_trust_binding(),"status_requests":queries.load(Ordering::SeqCst),"rejected":true}),
    );
    replacement.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}

async fn schema(admin: &mut sqlx::MySqlConnection, upgraded: bool) {
    admin
        .execute("DROP TABLE IF EXISTS seata_rs_tm_launcher_v1")
        .await
        .unwrap();
    for sql in [
        include_str!("../migrations/008_seata_rs_tm_launcher_v1.sql"),
        include_str!("../migrations/009_seata_rs_tm_launcher_owner_instance.sql"),
        include_str!("../migrations/010_seata_rs_tm_launcher_stop_scope.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&mut *admin).await.unwrap();
    }
    if upgraded {
        sqlx::raw_sql(include_str!(
            "../migrations/014_seata_rs_tm_transport_binding.sql"
        ))
        .execute(&mut *admin)
        .await
        .unwrap();
    }
}

struct ExitAfterIntent(MySqlLauncherStore);
impl LauncherStore for ExitAfterIntent {
    async fn record_begin(
        &self,
        origin: &LauncherOrigin,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.record_begin(origin).await
    }
    async fn record_end_intent(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.record_end_intent(origin, operation).await?;
        std::process::exit(0);
    }
    async fn mark_send_started(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.mark_send_started(origin, operation).await
    }
}

#[tokio::test]
#[ignore = "owned child entry; parent validates actual process stop proof"]
async fn binding_owner_child() {
    let folder = std::path::PathBuf::from(std::env::var("SEATA_TM_TRUST_MATERIAL").unwrap());
    let read = |name| std::fs::read(folder.join(name)).unwrap();
    let security = seata_transport::TlsClientConfig::new(
        "seata.example.test".into(),
        vec![read("root.der")],
        vec![read("client.der")],
        read("client.key.der"),
    )
    .unwrap();
    let address = std::env::var("SEATA_TM_TRUST_ENDPOINT")
        .unwrap()
        .parse()
        .unwrap();
    let manager = TransactionManager::builder(tls_peer::pool(address), identity())
        .transport_tls(security)
        .build()
        .unwrap();
    use std::io::Write;
    std::io::stdout().lock().write_all(b"\n").unwrap();
    manager
        .announce_supervised_instance(std::io::stdout().lock())
        .unwrap();
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = ExitAfterIntent(MySqlLauncherStore::new(source));
    let config = TransactionConfig::builder("bound-child".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    manager
        .begin_durable(&config, "trust-business", &store)
        .await
        .unwrap()
        .commit()
        .await
        .unwrap();
    panic!("owner must stop after durable intent and before send marker");
}

async fn serve_pending(
    stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
    queries: &AtomicU64,
    ends: &AtomicU64,
) {
    while let Some(request) = optional_frame(stream).await {
        let body = match request.body.unwrap() {
            Message::RegisterTMRequest(_) => {
                Message::RegisterTMResponse(seata_protocol::protocol::RegisterTMResponse {
                    identified: true,
                    wire_version: None,
                })
            }
            Message::GlobalBeginRequest(_) => peer::begin_result("tc:launcher-trust"),
            Message::GlobalStatusRequest(_) => {
                queries.fetch_add(1, Ordering::SeqCst);
                Message::GlobalStatusResponse(seata_protocol::transaction::GlobalStatusResponse {
                    response: peer::success(),
                    global_status: if ends.load(Ordering::SeqCst) == 0 {
                        GlobalStatus::Begin
                    } else {
                        GlobalStatus::Committed
                    },
                })
            }
            Message::GlobalCommitRequest(_) => {
                ends.fetch_add(1, Ordering::SeqCst);
                tm_fixture::end_response(true, GlobalStatus::Committed, peer::success())
            }
            other => panic!("unexpected recovery frame: {other:?}"),
        };
        tls_peer::write(stream, peer::reply(request.id, body)).await;
    }
}

#[tokio::test]
#[ignore = "requires owned MySQL and actual monitored mTLS owner process"]
async fn same_trust_stopped_owner_transfers_after_binding_race_is_rejected() {
    let mut admin = owned_mysql::admin().await;
    schema(&mut admin, true).await;
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(source.clone());
    let pki = tls_peer::Pki::new();
    let security = pki.client("seata.example.test");
    let binding = *security.peer_trust_binding();
    let acceptor = pki.acceptor();
    let folder = std::path::PathBuf::from(format!(
        "D:/codex-builds/tm-trust-owner-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    for (name, bytes) in [
        ("root.der", pki.root),
        ("client.der", pki.client_certificate),
        ("client.key.der", pki.client_key),
    ] {
        std::fs::write(folder.join(name), bytes).unwrap();
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let queries = Arc::new(AtomicU64::new(0));
    let ends = Arc::new(AtomicU64::new(0));
    let observed = queries.clone();
    let completed = ends.clone();
    let (stop, mut stopping) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(async move {
        let mut workers = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _=stopping.changed()=>break,
                accepted=listener.accept()=>{
                    let (socket,_)=accepted.unwrap();let acceptor=acceptor.clone();let observed=observed.clone();let completed=completed.clone();
                    workers.spawn(async move {if let Ok(mut stream)=acceptor.accept(socket).await {serve_pending(&mut stream,&observed,&completed).await;}});
                }
            }
        }
        while let Some(result) = workers.join_next().await {
            result.unwrap();
        }
    });
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--ignored", "--exact", "binding_owner_child", "--nocapture"])
        .env("SEATA_TM_TRUST_ENDPOINT", address.to_string())
        .env("SEATA_TM_TRUST_MATERIAL", &folder);
    let mut monitor = LauncherProcessMonitor::spawn(&mut command).unwrap();
    let stopped = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(proof) = monitor.try_confirm_exit().unwrap() {
                break proof;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let xid = seata_tm::Xid::parse("tc:launcher-trust".into()).unwrap();
    let snapshot = store.load_snapshot(&xid).await.unwrap().unwrap();
    assert_eq!(snapshot.peer_trust_binding(), Some(&binding));
    assert!(monitor.verifies(&stopped, snapshot.owner_instance_id()));
    assert_eq!(snapshot.end_operation(), Some(TmOperation::Commit));
    assert!(!snapshot.send_started());
    let (manager, authority) = TransactionManager::builder(tls_peer::pool(address), identity())
        .transport_tls(security)
        .build_with_recovered_end_authority()
        .unwrap();
    assert_eq!(manager.peer_trust_binding(), Some(&binding));
    let waits_seen = AtomicU64::new(0);
    // 真正取得源行锁，在接管者已进入 MySQL 锁等待后替换绑定，不能只依赖旧快照。
    use sqlx::Connection;
    {
        let mut blocker = admin.begin().await.unwrap();
        sqlx::query("SELECT xid_digest FROM seata_rs_tm_launcher_v1 FOR UPDATE")
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
        let mut observer = owned_mysql::admin().await;
        let claim =
            store.transfer_stopped_owner(&snapshot, &monitor, &stopped, &manager, "trust-business");
        tokio::pin!(claim);
        tokio::select! {
            result=&mut claim=>match result {Ok(_)=>panic!("claim succeeded before lock release"),Err(error)=>panic!("claim must wait for the source lock: {error:?}")},
            waited=tokio::time::timeout(Duration::from_secs(3),async {
                loop {
                    let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks r ON r.ENGINE=w.ENGINE AND r.ENGINE_LOCK_ID=w.REQUESTING_ENGINE_LOCK_ID WHERE r.OBJECT_SCHEMA=DATABASE() AND r.OBJECT_NAME='seata_rs_tm_launcher_v1'").fetch_one(&mut observer).await.unwrap();
                    if waits>0 {waits_seen.store(u64::try_from(waits).unwrap(),Ordering::SeqCst);break;}tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })=>{waited.unwrap();}
        }
        let wrong = [0x72u8; 32];
        sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET transport_binding=?")
            .bind(wrong.as_slice())
            .execute(&mut *blocker)
            .await
            .unwrap();
        blocker.commit().await.unwrap();
        assert!(
            claim.await.is_err(),
            "locked claim must compare persisted transport binding"
        );
    }
    let epoch: u64 = sqlx::query_scalar("SELECT owner_epoch FROM seata_rs_tm_launcher_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(epoch, 1);
    assert_eq!(ends.load(Ordering::SeqCst), 0);
    sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET transport_binding=?")
        .bind(binding.as_slice())
        .execute(&mut admin)
        .await
        .unwrap();
    let transferred = store
        .transfer_stopped_owner(&snapshot, &monitor, &stopped, &manager, "trust-business")
        .await
        .unwrap();
    assert_eq!(transferred.epoch(), 2);
    assert_eq!(
        transferred
            .finish(&authority, &store)
            .await
            .unwrap()
            .status(),
        GlobalStatus::Committed
    );
    let final_snapshot = store.load_snapshot(&xid).await.unwrap().unwrap();
    assert_eq!(final_snapshot.owner_epoch(), 2);
    assert!(final_snapshot.send_started());
    assert_eq!(final_snapshot.peer_trust_binding(), Some(&binding));
    assert_eq!(ends.load(Ordering::SeqCst), 1);
    assert!(queries.load(Ordering::SeqCst) >= 3);
    save(
        "claim-race.json",
        serde_json::json!({"observed_mysql_row_waits":waits_seen.load(Ordering::SeqCst),"epoch_after_rejection":epoch,"final_epoch":final_snapshot.owner_epoch(),"send_started":final_snapshot.send_started(),"commit_requests":ends.load(Ordering::SeqCst),"status_requests":queries.load(Ordering::SeqCst),"stored_binding":final_snapshot.peer_trust_binding(),"expected_binding":binding,"stopped_owner_verified":monitor.verifies(&stopped,snapshot.owner_instance_id())}),
    );
    manager.shutdown().await.unwrap();
    stop.send(true).unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned MySQL legacy schema and actual native peers"]
async fn legacy_plaintext_schema_stays_compatible_and_tls_requires_014() {
    let mut admin = owned_mysql::admin().await;
    schema(&mut admin, false).await;
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(source.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let queries = Arc::new(AtomicU64::new(0));
    let ends = Arc::new(AtomicU64::new(0));
    let observed = queries.clone();
    let ended = ends.clone();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        serve_pending(&mut stream, &observed, &ended).await;
    });
    let manager = TransactionManager::builder(tls_peer::pool(address), identity())
        .build()
        .unwrap();
    let config = TransactionConfig::builder("legacy-source".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let durable = manager
        .begin_durable(&config, "trust-business", &store)
        .await
        .unwrap();
    let xid = durable.xid().clone();
    let snapshot = store.load_snapshot(&xid).await.unwrap().unwrap();
    assert!(snapshot.peer_trust_binding().is_none());
    assert_eq!(
        snapshot
            .query_original_tc(&manager, "trust-business")
            .await
            .unwrap()
            .status(),
        GlobalStatus::Begin
    );
    durable.commit().await.unwrap();
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    assert_eq!(ends.load(Ordering::SeqCst), 1);
    sqlx::raw_sql(include_str!(
        "../migrations/014_seata_rs_tm_transport_binding.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let upgraded = store.load_snapshot(&xid).await.unwrap().unwrap();
    assert!(upgraded.peer_trust_binding().is_none());
    assert!(upgraded.send_started());
    let legacy_version: u16 =
        sqlx::query_scalar("SELECT schema_version FROM seata_rs_tm_launcher_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    schema(&mut admin, false).await;
    let pki = tls_peer::Pki::new();
    let security = pki.client("seata.example.test");
    let acceptor = pki.acceptor();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let ended = ends.clone();
    let observed = queries.clone();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut stream = acceptor.accept(socket).await.unwrap();
        serve_pending(&mut stream, &observed, &ended).await;
    });
    let manager = TransactionManager::builder(tls_peer::pool(address), identity())
        .transport_tls(security)
        .build()
        .unwrap();
    match manager
        .begin_durable(&config, "trust-business", &store)
        .await
    {
        Err(seata_tm::DurableTmError::Persistence {
            stage: "begin source",
            xid,
            source,
        }) => {
            assert_eq!(xid.as_str(), "tc:launcher-trust");
            assert!(
                matches!(
                    source.downcast_ref::<seata_db_mysql::DbError>(),
                    Some(seata_db_mysql::DbError::Driver {
                        code: Some(1054),
                        ..
                    })
                ),
                "missing 014 must expose original unknown-column failure"
            );
        }
        _ => panic!("missing 014 must not deliver a TLS business launcher"),
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tm_launcher_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(ends.load(Ordering::SeqCst), 1);
    save(
        "legacy.json",
        serde_json::json!({"legacy_schema_version":legacy_version,"legacy_binding":upgraded.peer_trust_binding(),"legacy_end_requests":ends.load(Ordering::SeqCst),"TLS_without_014_source_rows":count,"TLS_without_014_mysql_error":1054}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned MySQL and a live TLS source"]
async fn damaged_protected_binding_never_falls_back_and_original_bytes_allow_completion() {
    let mut admin = owned_mysql::admin().await;
    schema(&mut admin, true).await;
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(source.clone());
    let pki = tls_peer::Pki::new();
    let security = pki.client("seata.example.test");
    let binding = *security.peer_trust_binding();
    let acceptor = pki.acceptor();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let queries = Arc::new(AtomicU64::new(0));
    let ends = Arc::new(AtomicU64::new(0));
    let observed = queries.clone();
    let ended = ends.clone();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut stream = acceptor.accept(socket).await.unwrap();
        serve_pending(&mut stream, &observed, &ended).await;
    });
    let manager = TransactionManager::builder(tls_peer::pool(address), identity())
        .transport_tls(security)
        .build()
        .unwrap();
    let config = TransactionConfig::builder("damaged-source".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let durable = manager
        .begin_durable(&config, "trust-business", &store)
        .await
        .unwrap();
    let xid = durable.xid().clone();
    sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET transport_binding=?")
        .bind([0u8; 32].as_slice())
        .execute(&mut admin)
        .await
        .unwrap();
    let wrong = store.load_snapshot(&xid).await.unwrap().unwrap();
    assert!(matches!(
        wrong.query_original_tc(&manager, "trust-business").await,
        Err(TmError::IllegalState { .. })
    ));
    assert_eq!(queries.load(Ordering::SeqCst), 0);
    sqlx::raw_sql("ALTER TABLE seata_rs_tm_launcher_v1 DROP CHECK chk_tm_launcher_transport_binding, DROP COLUMN transport_binding").execute(&mut admin).await.unwrap();
    assert!(
        store.load_snapshot(&xid).await.is_err(),
        "protected version with missing column cannot become plaintext"
    );
    sqlx::raw_sql(
        "ALTER TABLE seata_rs_tm_launcher_v1 ADD COLUMN transport_binding VARBINARY(64) NULL",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    assert!(
        store.load_snapshot(&xid).await.is_err(),
        "NULL protected binding must fail closed"
    );
    sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET transport_binding=?")
        .bind(vec![0u8; 31])
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(store.load_snapshot(&xid).await.is_err());
    sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET transport_binding=?")
        .bind(binding.as_slice())
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql("ALTER TABLE seata_rs_tm_launcher_v1 MODIFY transport_binding BINARY(32) NULL, ADD CONSTRAINT chk_tm_launcher_transport_binding CHECK ((schema_version=1 AND transport_binding IS NULL) OR (schema_version=2 AND transport_binding IS NOT NULL))").execute(&mut admin).await.unwrap();
    let restored = store.load_snapshot(&xid).await.unwrap().unwrap();
    assert_eq!(restored.peer_trust_binding(), Some(&binding));
    assert_eq!(
        restored
            .query_original_tc(&manager, "trust-business")
            .await
            .unwrap()
            .status(),
        GlobalStatus::Begin
    );
    durable.commit().await.unwrap();
    assert_eq!(ends.load(Ordering::SeqCst), 1);
    assert_eq!(queries.load(Ordering::SeqCst), 1);
    save(
        "restored.json",
        serde_json::json!({"wrong_binding_status_requests":0,"missing_column_rejected":true,"null_rejected":true,"short_binding_rejected":true,"restored_binding":restored.peer_trust_binding(),"expected_binding":binding,"status_requests_after_restore":queries.load(Ordering::SeqCst),"commit_requests_after_restore":ends.load(Ordering::SeqCst)}),
    );
    manager.shutdown().await.unwrap();
    server.await.unwrap();
    source.close().await.unwrap();
}

async fn denied(connection: &mut sqlx::MySqlConnection, sql: &str) {
    let error = sqlx::query(sql).execute(connection).await.unwrap_err();
    let code = error
        .as_database_error()
        .and_then(|e| e.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
        .map(sqlx::mysql::MySqlDatabaseError::number);
    assert!(
        matches!(code, Some(1142 | 1143)),
        "unexpected privilege failure: {error}"
    );
}
async fn account(
    admin: &mut sqlx::MySqlConnection,
    name: &str,
    role: &str,
) -> sqlx::MySqlConnection {
    use sqlx::{Connection, Row};
    let row = sqlx::query(&format!(
        "CREATE USER '{name}'@'%' IDENTIFIED BY RANDOM PASSWORD"
    ))
    .fetch_one(&mut *admin)
    .await
    .unwrap();
    let password: String = row.get(2);
    sqlx::raw_sql(&format!(
        "GRANT '{role}' TO '{name}'@'%';SET DEFAULT ROLE '{role}' TO '{name}'@'%'"
    ))
    .execute(&mut *admin)
    .await
    .unwrap();
    sqlx::MySqlConnection::connect_with(
        &sqlx::mysql::MySqlConnectOptions::new()
            .host("127.0.0.1")
            .port(std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap())
            .database(&std::env::var("SEATA_MYSQL_DATABASE").unwrap())
            .username(name)
            .password(&password)
            .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires owned MySQL role administration"]
async fn transport_binding_grants_allow_insert_but_no_role_can_rewrite_source_trust() {
    let mut admin = owned_mysql::admin().await;
    schema(&mut admin, true).await;
    sqlx::raw_sql("DROP USER IF EXISTS 'tm_trust_writer'@'%', 'tm_trust_recovery'@'%'; DROP ROLE IF EXISTS 'seata_tm_launcher_reader','seata_tm_launcher_writer','seata_tm_launcher_recovery'").execute(&mut admin).await.unwrap();
    sqlx::raw_sql(include_str!("../sql/tm_launcher_roles.sql"))
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../sql/tm_launcher_recovery_role.sql"))
        .execute(&mut admin)
        .await
        .unwrap();
    let mut writer = account(&mut admin, "tm_trust_writer", "seata_tm_launcher_writer").await;
    let mut recovery = account(
        &mut admin,
        "tm_trust_recovery",
        "seata_tm_launcher_recovery",
    )
    .await;
    let insert="INSERT INTO seata_rs_tm_launcher_v1(schema_version,xid_digest,xid,tc_endpoint,application_id,service_group,begin_exception_code,business_intent,owner_instance_id,transport_binding,deadline_at,created_at,modified_at) VALUES(2,UNHEX(SHA2('tc:role-binding',256)),'tc:role-binding','127.0.0.1:8091','app','group',0,'trust-role',REPEAT('a',32),REPEAT('b',32),DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 60 SECOND),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))";
    denied(&mut writer, insert).await;
    sqlx::raw_sql(include_str!(
        "../sql/tm_launcher_transport_binding_upgrade_grants.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    writer.execute(insert).await.unwrap();
    denied(
        &mut writer,
        "UPDATE seata_rs_tm_launcher_v1 SET transport_binding=REPEAT('c',32)",
    )
    .await;
    denied(
        &mut writer,
        "UPDATE seata_rs_tm_launcher_v1 SET schema_version=1",
    )
    .await;
    denied(
        &mut recovery,
        "UPDATE seata_rs_tm_launcher_v1 SET transport_binding=REPEAT('c',32)",
    )
    .await;
    denied(&mut recovery, insert).await;
    recovery
        .execute("UPDATE seata_rs_tm_launcher_v1 SET owner_epoch=2")
        .await
        .unwrap();
    let trust: Vec<u8> =
        sqlx::query_scalar("SELECT transport_binding FROM seata_rs_tm_launcher_v1")
            .fetch_one(&mut recovery)
            .await
            .unwrap();
    assert_eq!(trust, vec![b'b'; 32]);
    use sqlx::Connection;
    save(
        "grants.json",
        serde_json::json!({"insert_rejected_before_upgrade":true,"immutable_binding_updates_denied":2,"schema_version_update_denied":true,"recovery_insert_denied":true,"binding_after_allowed_epoch_update":trust}),
    );
    writer.close().await.unwrap();
    recovery.close().await.unwrap();
}
