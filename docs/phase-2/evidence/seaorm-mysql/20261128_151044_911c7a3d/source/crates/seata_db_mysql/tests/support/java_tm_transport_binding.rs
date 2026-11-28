//! 固定原 TC 的持久 TLS 来源与信任扩大拒绝，不替代原结束/Status 帧断言。
pub(super) async fn verify(
    store: &seata_db_mysql::MySqlLauncherStore,
    snapshot: &seata_db_mysql::MySqlLauncherSnapshot,
    observer: &seata_tm::TransactionManager,
    expected: &[u8; 32],
    pool: seata_transport::ConnectionPoolConfig,
    identity: seata_transport::RegistrationIdentity,
) {
    use seata_tm::{TmError, TmOperation, TransactionManager};
    let material = std::path::PathBuf::from(std::env::var("SEATA_TRANSPORT_TLS_MATERIAL").unwrap());
    let read = |name| std::fs::read(material.join(name)).unwrap();
    let wider = seata_transport::TlsClientConfig::new(
        "seata.example.test".into(),
        vec![read("root.der"), read("other-root.der")],
        vec![read("client.der")],
        read("client.key.der"),
    )
    .unwrap();
    assert_ne!(wider.peer_trust_binding(), expected);
    let candidate = TransactionManager::builder(pool, identity)
        .transport_tls(wider)
        .build()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while candidate.connection_snapshot().ready() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        snapshot
            .query_original_tc(&candidate, "order-original-tc")
            .await,
        Err(TmError::IllegalState { .. })
    ));
    let calls = candidate.metrics_sampler().snapshot(TmOperation::Status);
    assert_eq!(calls.active, 0);
    assert_eq!(calls.observed, 0);
    assert_eq!(calls.unknown, 0);
    assert_eq!(calls.not_sent, 0);
    let wrong = *candidate.peer_trust_binding().unwrap();
    candidate.shutdown().await.unwrap();
    assert_eq!(snapshot.peer_trust_binding(), Some(expected));
    assert_eq!(observer.peer_trust_binding(), Some(expected));
    let loaded = store.load_snapshot(snapshot.xid()).await.unwrap().unwrap();
    assert_eq!(loaded.peer_trust_binding(), Some(expected));
    let mut admin = super::owned_mysql::admin().await;
    let (version, binding): (u16, Vec<u8>) =
        sqlx::query_as("SELECT schema_version,transport_binding FROM seata_rs_tm_launcher_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!(version, 2);
    assert_eq!(binding, expected);
    std::fs::write(super::observation_dir().join("tm-transport-binding.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "xid":snapshot.xid().as_str(),"source_schema_version":version,"stored_binding":binding,
        "observer_binding":expected,"wrong_trust_binding":wrong,"wrong_trust_rejected":true,
        "wrong_trust_status_calls":calls.observed+calls.unknown+calls.not_sent,"original_end_send_marker":loaded.send_started(),
    })).unwrap()).unwrap();
}
