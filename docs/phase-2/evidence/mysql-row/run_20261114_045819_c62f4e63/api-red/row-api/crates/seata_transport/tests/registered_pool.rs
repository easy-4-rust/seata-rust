//! 新会话注册与版本化资源ready的真实TCP验收；并非生产认证策略或RM业务执行证明。
#![allow(clippy::unwrap_used)]
#[path = "support/session_peer.rs"]
mod peer;
use peer::{begin, begin_result, config, eof, read, reply, write, xid};
use seata_protocol::protocol::{RegisterRMResponse, RegisterTMResponse};
use seata_protocol::{BranchType, Message, RequestType};
use seata_transport::{
    CallError, ConnectionPool, ConnectionPoolConfig, PoolSlotState, ReconnectPolicy,
    RegistrationIdentity, RegistrationStatus, ResourceKey, ResourceRegistry, SessionRegistration,
    TransportError,
};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn settings(address: std::net::SocketAddr) -> ConnectionPoolConfig {
    ConnectionPoolConfig::new(
        address,
        config(8),
        1,
        4,
        ReconnectPolicy::new(
            Duration::from_millis(20),
            Duration::from_millis(80),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
fn identity() -> RegistrationIdentity {
    RegistrationIdentity::new(
        "2.8.0".into(),
        "rust-registration-fixture".into(),
        "default_tx_group".into(),
        None,
    )
    .unwrap()
}
fn registry(names: &[&str]) -> ResourceRegistry {
    let cache = ResourceRegistry::new(8, 512).unwrap();
    for name in names {
        cache
            .insert(ResourceKey::new(BranchType::At, (*name).into()).unwrap())
            .unwrap();
    }
    cache
}
fn rm_result(accepted: bool) -> Message {
    Message::RegisterRMResponse(RegisterRMResponse {
        identified: accepted,
        wire_version: None,
    })
}
fn tm_result(accepted: bool) -> Message {
    Message::RegisterTMResponse(RegisterTMResponse {
        identified: accepted,
        wire_version: Some("2.8.0".into()),
    })
}
async fn rm_step(socket: &mut tokio::net::TcpStream, resource: Option<&str>, accepted: bool) {
    let request = read(socket).await;
    match &request.body {
        Some(Message::RegisterRMRequest(value)) => {
            assert_eq!(value.resource_ids.as_deref(), resource);
            assert_eq!(value.version, "2.8.0");
            assert_eq!(value.extra_data, None);
        }
        other => panic!("expected RM registration: {other:?}"),
    };
    write(socket, reply(request.id, rm_result(accepted))).await;
}

#[test]
fn registry_is_bounded_typed_idempotent_and_redacts_debug() {
    let cache = ResourceRegistry::new(2, 8).unwrap();
    let at = ResourceKey::new(BranchType::At, "secret".into()).unwrap();
    assert!(cache.insert(at.clone()).unwrap());
    let revision = cache.snapshot().unwrap().revision();
    assert!(!cache.insert(at).unwrap());
    assert_eq!(cache.snapshot().unwrap().revision(), revision);
    assert!(cache
        .insert(ResourceKey::new(BranchType::Xa, "secret".into()).unwrap())
        .unwrap());
    assert_eq!(cache.snapshot().unwrap().resources().len(), 2);
    assert!(cache
        .insert(ResourceKey::new(BranchType::At, "third".into()).unwrap())
        .is_err());
    assert!(ResourceKey::new(BranchType::At, "a,b".into()).is_err());
    assert!(ResourceKey::new(BranchType::At, "".into()).is_err());
    assert!(ResourceRegistry::new(0, 1).is_err());
    assert!(ResourceRegistry::new(1, 0).is_err());
    let text = format!("{cache:?} {:?}", cache.snapshot().unwrap());
    assert!(!text.contains("secret"));
    let credentials = RegistrationIdentity::new(
        "2.8.0".into(),
        "private-app".into(),
        "private-group".into(),
        Some("private-credential".into()),
    )
    .unwrap();
    assert!(!format!("{credentials:?}").contains("private"));
}

#[test]
fn registration_identity_and_registry_byte_budgets_fail_before_mutation() {
    let cache = ResourceRegistry::new(2, 2).unwrap();
    assert!(cache
        .insert(ResourceKey::new(BranchType::At, "中文".into()).unwrap())
        .is_err());
    assert_eq!(cache.snapshot().unwrap().revision(), 0);
    assert!(cache.snapshot().unwrap().resources().is_empty());
    assert!(RegistrationIdentity::new("".into(), "app".into(), "group".into(), None).is_err());
    assert!(RegistrationIdentity::new(
        "2.8.0".into(),
        "app".into(),
        "group".into(),
        Some("x".repeat(32768))
    )
    .is_err());
    assert!(SessionRegistration::tm(identity(), Duration::ZERO).is_err());
}

#[tokio::test]
async fn explicit_identity_denial_is_rejected_without_resource_or_business_attempt() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read(&mut socket).await;
        write(&mut socket, reply(request.id, tm_result(false))).await;
        eof(&mut socket).await;
    });
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::tm(identity(), Duration::from_secs(1)).unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    let report = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(report) = handle.snapshot().slots[0].registration.clone() {
                if report.identity.status == RegistrationStatus::Rejected {
                    break report;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(report.identity.error.is_none());
    assert!(!report.is_ready());
    assert_eq!(handle.snapshot().ready(), 0);
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn registry_change_cancels_outdated_initialization_and_late_reply_cannot_publish_ready() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (seen_tx, seen_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let stale = read(&mut socket).await;
        seen_tx.send(()).unwrap();
        let current = read(&mut socket).await;
        assert_ne!(stale.id, current.id);
        write(&mut socket, reply(stale.id, rm_result(true))).await;
        write(&mut socket, reply(current.id, rm_result(true))).await;
        rm_step(&mut socket, Some("db-b"), true).await;
        let request = read(&mut socket).await;
        write(&mut socket, reply(request.id, begin_result("current-plan"))).await;
        eof(&mut socket).await;
    });
    let cache = registry(&["db-a"]);
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::rm(identity(), cache.clone(), Duration::from_secs(1)).unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    seen_rx.await.unwrap();
    cache
        .insert(ResourceKey::new(BranchType::At, "db-b".into()).unwrap())
        .unwrap();
    assert_eq!(handle.snapshot().ready(), 0);
    assert_eq!(
        xid(handle
            .request(begin("after-change"), Duration::from_secs(2))
            .await
            .unwrap()),
        "current-plan"
    );
    let current = handle.acquire(Duration::from_secs(1)).await.unwrap();
    assert!(current.snapshot().unmatched_responses >= 1);
    assert_eq!(
        handle.snapshot().slots[0]
            .registration
            .as_ref()
            .unwrap()
            .registry_revision,
        cache.snapshot().unwrap().revision()
    );
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn invalid_resource_frame_is_local_registration_failure_not_unknown_or_rejection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        rm_step(&mut socket, Some("db-a"), true).await;
        eof(&mut socket).await;
    });
    let frames = seata_protocol::v1::FrameLimits::new(
        128,
        64,
        8,
        seata_protocol::PayloadLimits::new(128, 32, 64, 8).unwrap(),
    )
    .unwrap();
    let connection = seata_transport::ConnectionConfig::new(
        frames,
        4,
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let configuration = ConnectionPoolConfig::new(
        address,
        connection,
        1,
        4,
        ReconnectPolicy::new(
            Duration::from_millis(20),
            Duration::from_millis(80),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap();
    let cache = registry(&["db-a", &"x".repeat(200)]);
    let pool = ConnectionPool::start_registered(
        configuration,
        SessionRegistration::rm(identity(), cache, Duration::from_secs(1)).unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    let report = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(report) = handle.snapshot().slots[0].registration.clone() {
                if report.resources.get(1).is_some_and(|value| {
                    value.attempt.status == RegistrationStatus::LocalFailureBeforeSend
                }) {
                    break report;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!report.resources[1]
        .attempt
        .error
        .as_ref()
        .unwrap()
        .is_unknown_outcome());
    assert_eq!(handle.snapshot().ready(), 0);
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn wrong_registration_response_family_preserves_unknown_protocol_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read(&mut socket).await;
        write(&mut socket, reply(request.id, tm_result(true))).await;
        eof(&mut socket).await;
    });
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::rm(identity(), registry(&["db-a"]), Duration::from_secs(1)).unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    let report = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(report) = handle.snapshot().slots[0].registration.clone() {
                if report.identity.status == RegistrationStatus::UnknownOutcome {
                    break report;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        report.identity.error,
        Some(CallError::UnknownOutcome {
            reason: TransportError::UnexpectedResponse { .. },
            ..
        })
    ));
    assert_eq!(handle.snapshot().ready(), 0);
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn rm_first_registration_includes_resource_for_identity_and_resource_auth_policy() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let first = read(&mut socket).await;
        let allowed = matches!(first.body,Some(Message::RegisterRMRequest(ref value)) if value.resource_ids.as_deref()==Some("mysql://auth-a"));
        write(&mut socket, reply(first.id, rm_result(allowed))).await;
        if !allowed {
            eof(&mut socket).await;
            return;
        }
        rm_step(&mut socket, Some("mysql://auth-b"), true).await;
        eof(&mut socket).await;
    });
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::rm(
            identity(),
            registry(&["mysql://auth-a", "mysql://auth-b"]),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap();
    let result = pool.handle().acquire(Duration::from_secs(1)).await;
    assert!(
        result.is_ok(),
        "RM first request must also satisfy a nonempty-resource authentication policy"
    );
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn withheld_tm_identity_prevents_business_write_until_identified() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (seen_tx, seen_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let registration = read(&mut socket).await;
        assert!(
            matches!(registration.body,Some(Message::RegisterTMRequest(ref value)) if value.extra_data.is_empty() && value.version=="2.8.0")
        );
        seen_tx.send(()).unwrap();
        release_rx.await.unwrap();
        write(&mut socket, reply(registration.id, tm_result(true))).await;
        let request = read(&mut socket).await;
        assert!(matches!(request.body, Some(Message::GlobalBeginRequest(_))));
        write(
            &mut socket,
            peer::heartbeat(request.id, RequestType::HeartbeatRequest),
        )
        .await;
        assert_eq!(
            read(&mut socket).await.message_type,
            RequestType::HeartbeatResponse
        );
        write(
            &mut socket,
            reply(request.id, begin_result("identity-ready")),
        )
        .await;
        eof(&mut socket).await;
    });
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::tm(identity(), Duration::from_secs(1)).unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    let caller = handle.clone();
    let call = tokio::spawn(async move {
        caller
            .request(begin("after-identity"), Duration::from_secs(2))
            .await
    });
    seen_rx.await.unwrap();
    assert_eq!(handle.snapshot().ready(), 0);
    assert_eq!(handle.snapshot().slots[0].state, PoolSlotState::Registering);
    assert!(matches!(
        handle.acquire(Duration::from_millis(40)).await,
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::PoolAcquireTimeout,
            ..
        })
    ));
    release_tx.send(()).unwrap();
    assert_eq!(xid(call.await.unwrap().unwrap()), "identity-ready");
    assert_eq!(handle.snapshot().ready(), 1);
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn partial_resource_denial_stays_degraded_with_every_required_outcome() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        rm_step(&mut socket, Some("db-a"), true).await;
        rm_step(&mut socket, Some("db-b"), false).await;
        eof(&mut socket).await;
    });
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::rm(
            identity(),
            registry(&["db-a", "db-b", "db-c"]),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    let report = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(report) = handle.snapshot().slots[0].registration.clone() {
                if report
                    .resources
                    .get(1)
                    .is_some_and(|entry| entry.attempt.status == RegistrationStatus::Rejected)
                {
                    break report;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!report.is_ready());
    assert_eq!(report.identity.status, RegistrationStatus::Accepted);
    assert_eq!(
        report.resources[0].attempt.status,
        RegistrationStatus::Accepted
    );
    assert_eq!(
        report.resources[1].attempt.status,
        RegistrationStatus::Rejected
    );
    assert_eq!(
        report.resources[2].attempt.status,
        RegistrationStatus::NotAttempted
    );
    assert_eq!(report.resources[1].resource.resource_id(), "db-b");
    assert_eq!(handle.snapshot().ready(), 0);
    assert!(matches!(
        handle.acquire(Duration::from_millis(40)).await,
        Err(CallError::LocalFailureBeforeSend { .. })
    ));
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn replacement_session_repeats_identity_and_all_cached_resources_before_status() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for generation in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            rm_step(&mut socket, Some("db-a"), true).await;
            rm_step(&mut socket, Some("db-b"), true).await;
            let business = read(&mut socket).await;
            if generation == 0 {
                assert!(matches!(
                    business.body,
                    Some(Message::GlobalBeginRequest(_))
                ));
                drop(socket);
            } else {
                assert!(matches!(
                    business.body,
                    Some(Message::GlobalStatusRequest(_))
                ));
                write(&mut socket, reply(business.id, peer::status_result())).await;
                eof(&mut socket).await;
            }
        }
    });
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::rm(
            identity(),
            registry(&["db-a", "db-b"]),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    let old = handle.acquire(Duration::from_secs(2)).await.unwrap();
    assert!(handle
        .request(begin("lost-no-replay"), Duration::from_secs(2))
        .await
        .unwrap_err()
        .is_unknown_outcome());
    let response = handle
        .request(
            Message::GlobalStatusRequest(seata_protocol::transaction::GlobalStatusRequest {
                xid: Some("original".into()),
                extra_data: None,
            }),
            Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert!(matches!(
        response.body,
        Some(Message::GlobalStatusResponse(_))
    ));
    let new = handle.acquire(Duration::from_secs(1)).await.unwrap();
    assert_ne!(old.generation(), new.generation());
    assert!(old.snapshot().closed);
    assert_eq!(handle.snapshot().ready(), 1);
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn adding_resource_invalidates_old_revision_until_live_session_is_registered_again() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (seen_tx, seen_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        rm_step(&mut socket, Some("db-a"), true).await;
        let first = read(&mut socket).await;
        write(&mut socket, reply(first.id, begin_result("before-add"))).await;
        rm_step(&mut socket, Some("db-a"), true).await;
        let added = read(&mut socket).await;
        assert!(
            matches!(added.body,Some(Message::RegisterRMRequest(ref value)) if value.resource_ids.as_deref()==Some("db-b"))
        );
        seen_tx.send(()).unwrap();
        release_rx.await.unwrap();
        write(&mut socket, reply(added.id, rm_result(true))).await;
        let last = read(&mut socket).await;
        write(&mut socket, reply(last.id, begin_result("after-add"))).await;
        eof(&mut socket).await;
    });
    let cache = registry(&["db-a"]);
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::rm(identity(), cache.clone(), Duration::from_secs(1)).unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    assert_eq!(
        xid(handle
            .request(begin("before"), Duration::from_secs(2))
            .await
            .unwrap()),
        "before-add"
    );
    let old = handle.acquire(Duration::from_secs(1)).await.unwrap();
    assert!(cache
        .insert(ResourceKey::new(BranchType::At, "db-b".into()).unwrap())
        .unwrap());
    assert_eq!(handle.snapshot().ready(), 0);
    seen_rx.await.unwrap();
    assert_eq!(handle.snapshot().ready(), 0);
    assert!(handle.acquire(Duration::from_millis(40)).await.is_err());
    release_tx.send(()).unwrap();
    assert_eq!(
        xid(handle
            .request(begin("after"), Duration::from_secs(2))
            .await
            .unwrap()),
        "after-add"
    );
    assert_eq!(
        handle
            .acquire(Duration::from_secs(1))
            .await
            .unwrap()
            .generation(),
        old.generation()
    );
    let report = handle.snapshot().slots[0].registration.clone().unwrap();
    assert_eq!(
        report.registry_revision,
        cache.snapshot().unwrap().revision()
    );
    assert!(report.is_ready());
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn lost_identity_response_is_unknown_registration_not_explicit_rejection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        assert!(matches!(
            read(&mut socket).await.body,
            Some(Message::RegisterRMRequest(_))
        ));
        eof(&mut socket).await;
    });
    let pool = ConnectionPool::start_registered(
        settings(address),
        SessionRegistration::rm(identity(), registry(&["db-a"]), Duration::from_millis(80))
            .unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    let report = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(report) = handle.snapshot().slots[0].registration.clone() {
                if report.identity.status == RegistrationStatus::UnknownOutcome {
                    break report;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(report.identity.error.as_ref().unwrap().is_unknown_outcome());
    assert_eq!(
        report.resources[0].attempt.status,
        RegistrationStatus::UnknownOutcome
    );
    assert_eq!(handle.snapshot().ready(), 0);
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn shutdown_and_drop_cancel_identity_wait_and_close_socket() {
    for abrupt in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (seen_tx, seen_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read(&mut socket).await;
            seen_tx.send(()).unwrap();
            eof(&mut socket).await;
        });
        let pool = ConnectionPool::start_registered(
            settings(address),
            SessionRegistration::rm(identity(), registry(&["db-a"]), Duration::from_secs(5))
                .unwrap(),
        )
        .unwrap();
        let handle = pool.handle();
        seen_rx.await.unwrap();
        assert_eq!(handle.snapshot().ready(), 0);
        if abrupt {
            drop(pool);
        } else {
            tokio::time::timeout(Duration::from_millis(500), pool.shutdown())
                .await
                .unwrap()
                .unwrap();
        }
        assert!(handle.snapshot().closed);
        server.await.unwrap();
    }
}
