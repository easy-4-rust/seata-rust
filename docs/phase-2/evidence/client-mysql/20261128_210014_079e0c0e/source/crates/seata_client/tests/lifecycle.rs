//! 实际 TCP 登记与真实日志租约验证；不代表原 Java TC 联试。
#![allow(clippy::unwrap_used)]
mod support;
use seata_client::{SeataClient, SeataClientBuilder};
use seata_protocol::protocol::{RegisterRMResponse, RegisterTMResponse};
use seata_protocol::{BranchType, Message};
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, FileBranchStore, ResourceCatalog};
use seata_transport::ResourceKey;
use std::time::Duration;
use support::peer;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

fn builder(listener: &TcpListener) -> SeataClientBuilder {
    SeataClient::builder(
        support::connection(listener.local_addr().unwrap()),
        support::identity(),
    )
}

#[tokio::test]
async fn rejects_invalid_configuration_before_any_dial() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cases = vec![
        builder(&listener).allow_plaintext(),
        builder(&listener).enable_tm(),
    ];
    for timeout in [Duration::ZERO, Duration::MAX] {
        cases.push(
            builder(&listener)
                .enable_tm()
                .allow_plaintext()
                .request_timeout(timeout),
        );
        cases.push(
            builder(&listener)
                .enable_tm()
                .allow_plaintext()
                .registration_timeout(timeout),
        );
        cases.push(
            builder(&listener)
                .enable_tm()
                .allow_plaintext()
                .callback_timeout(timeout),
        );
    }
    for maximum in [0, 4097] {
        cases.push(
            builder(&listener)
                .enable_tm()
                .allow_plaintext()
                .maximum_workers(maximum),
        );
    }
    for maximum in [0, 257] {
        cases.push(
            builder(&listener)
                .enable_tm()
                .allow_plaintext()
                .maximum_batch(maximum),
        );
    }
    for case in cases {
        let debug = format!("{case:?}");
        for secret in [
            "client-app-secret",
            "client-group-secret",
            "credential-secret",
        ] {
            assert!(!debug.contains(secret));
        }
        let error = case.build().await.unwrap_err();
        assert!(error.configuration_error().is_some());
        assert!(error.tm_error().is_none() && error.rm_error().is_none());
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(80), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn all_role_combinations_register_same_identity_and_shutdown_real_sockets() {
    for (tm, rm) in [(true, false), (false, true), (true, true)] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut configured = builder(&listener)
            .allow_plaintext()
            .launcher_stop_scope([7; 32]);
        let mut journal_path = None;
        if tm {
            configured = configured.enable_tm();
        }
        if rm {
            let (path, store) = support::journal().await;
            journal_path = Some(path);
            configured = configured.enable_rm(ResourceCatalog::new(4, 4096).unwrap(), store);
        }
        let client = configured.build().await.unwrap();
        assert_eq!(client.transaction_manager().is_some(), tm);
        assert_eq!(client.resource_manager().is_some(), rm);
        if let Some(manager) = client.transaction_manager() {
            assert_eq!(manager.owner_stop_scope(), Some(&[7; 32]));
        }
        let mut streams = Vec::new();
        let mut observed = (false, false);
        for _ in 0..usize::from(tm) + usize::from(rm) {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let frame = peer::read(&mut socket).await;
            let response = match frame.body.unwrap() {
                Message::RegisterTMRequest(request) => {
                    assert!(!observed.0);
                    observed.0 = true;
                    assert_eq!(request.version, "1.1.0");
                    assert_eq!(request.application_id, "client-app-secret");
                    assert_eq!(request.transaction_service_group, "client-group-secret");
                    assert_eq!(request.extra_data, "credential-secret");
                    Message::RegisterTMResponse(RegisterTMResponse {
                        identified: true,
                        wire_version: None,
                    })
                }
                Message::RegisterRMRequest(request) => {
                    assert!(!observed.1);
                    observed.1 = true;
                    assert_eq!(request.version, "1.1.0");
                    assert_eq!(request.application_id.as_deref(), Some("client-app-secret"));
                    assert_eq!(request.transaction_service_group, "client-group-secret");
                    assert_eq!(request.extra_data.as_deref(), Some("credential-secret"));
                    Message::RegisterRMResponse(RegisterRMResponse {
                        identified: true,
                        wire_version: None,
                    })
                }
                other => panic!("unexpected business message: {other:?}"),
            };
            peer::write(&mut socket, peer::reply(frame.id, response)).await;
            streams.push(socket);
        }
        assert_eq!(observed, (tm, rm));
        let samplers: Vec<_> = client
            .transaction_manager()
            .map(|m| m.connection_sampler())
            .into_iter()
            .chain(client.resource_manager().map(|m| m.connection_sampler()))
            .collect();
        tokio::time::timeout(Duration::from_secs(3), async {
            while samplers.iter().any(|s| s.snapshot().ready() != 1) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let debug = format!("{client:?}");
        assert!(!debug.contains("credential-secret"));
        client.shutdown().await.unwrap();
        assert!(samplers.iter().all(|s| s.snapshot().closed));
        for mut socket in streams {
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), socket.read(&mut [0; 1]))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        }
        if let Some(path) = journal_path {
            drop(
                FileBranchStore::open(path.clone(), support::options())
                    .await
                    .unwrap(),
            );
            std::fs::remove_file(path).unwrap();
        }
    }
}

#[tokio::test]
async fn shutdown_reports_held_lease_and_drop_preserves_pending_for_reopen() {
    for shutdown_mode in [0, 1, 2] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (path, store) = support::journal().await;
        let identity = BranchIdentity::new(
            "tc:9007199254740993".into(),
            9007199254740995,
            ResourceKey::new(BranchType::At, "mysql://exact-resource".into()).unwrap(),
            Some("original".into()),
        )
        .unwrap();
        let mut lease = store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap();
        lease.mark_pending().await.unwrap();
        let client = builder(&listener)
            .enable_tm()
            .enable_rm(ResourceCatalog::new(4, 4096).unwrap(), store)
            .allow_plaintext()
            .callback_timeout(Duration::from_millis(50))
            .build()
            .await
            .unwrap();
        let tm = client.transaction_manager().unwrap().connection_sampler();
        let rm = client.resource_manager().unwrap().connection_sampler();
        if shutdown_mode == 1 {
            let error = tokio::time::timeout(Duration::from_secs(2), client.shutdown())
                .await
                .unwrap()
                .unwrap_err();
            assert!(error.tm_error().is_none());
            assert!(error.rm_error().is_some());
        } else if shutdown_mode == 2 {
            assert!(
                tokio::time::timeout(Duration::from_millis(5), client.shutdown())
                    .await
                    .is_err()
            );
        } else {
            drop(client);
        }
        assert!(tm.snapshot().closed && rm.snapshot().closed);
        drop(lease);
        // Drop 发出关闭后，网络任务需要获得一次调度以释放自己的日志克隆。
        let reopened = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                match FileBranchStore::open(path.clone(), support::options()).await {
                    Ok(store) => break store,
                    Err(_) => tokio::task::yield_now().await,
                }
            }
        })
        .await
        .unwrap();
        let lease = reopened
            .lease(identity, BranchPhase::Rollback)
            .await
            .unwrap();
        assert_eq!(lease.state(), Some(BranchRecordState::Pending));
        drop(lease);
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
}
