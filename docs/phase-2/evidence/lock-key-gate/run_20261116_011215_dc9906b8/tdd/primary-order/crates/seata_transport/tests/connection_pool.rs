//! 有界原生TCP池与重连；Connected只表示raw通道，不代表认证或资源ready。
#![allow(clippy::unwrap_used)]
#[path = "support/session_peer.rs"]
mod peer;
use peer::{begin, begin_result, config, eof, read, reply, write, xid};
use seata_protocol::transaction::GlobalStatusRequest;
use seata_protocol::{Message, RequestType};
use seata_transport::{
    CallError, ConnectionPool, ConnectionPoolConfig, ReconnectPolicy, TransportError,
};
use std::time::Duration;
use tokio::net::{TcpListener, TcpSocket};
use tokio::sync::oneshot;

fn pool_config(
    endpoint: std::net::SocketAddr,
    slots: usize,
    inbound: usize,
) -> ConnectionPoolConfig {
    ConnectionPoolConfig::new(
        endpoint,
        config(8),
        slots,
        inbound,
        ReconnectPolicy::new(
            Duration::from_millis(20),
            Duration::from_millis(80),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn pool_budgets_and_backoff_are_explicit_and_validated() {
    let endpoint = "127.0.0.1:8080".parse().unwrap();
    let policy = ReconnectPolicy::new(
        Duration::from_millis(10),
        Duration::from_secs(1),
        Duration::from_secs(5),
    )
    .unwrap();
    for (slots, inbound) in [(0, 1), (1025, 1), (1, 0)] {
        assert!(ConnectionPoolConfig::new(endpoint, config(1), slots, inbound, policy).is_err());
    }
    for (base, max, stable) in [
        (
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::from_secs(1),
        ),
        (
            Duration::from_secs(2),
            Duration::from_secs(1),
            Duration::from_secs(1),
        ),
        (
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::ZERO,
        ),
        (
            Duration::from_secs(1),
            Duration::MAX,
            Duration::from_secs(1),
        ),
    ] {
        assert!(ReconnectPolicy::new(base, max, stable).is_err());
    }
}
#[tokio::test]
async fn two_slots_are_reused_by_concurrent_acquisition_without_creating_new_sockets() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (ready_tx, ready_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut a, _) = listener.accept().await.unwrap();
        let (mut b, _) = listener.accept().await.unwrap();
        ready_tx.send(()).unwrap();
        let (x, y) = tokio::join!(eof(&mut a), eof(&mut b));
        let _ = (x, y);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    });
    let pool = ConnectionPool::start(pool_config(addr, 2, 4)).unwrap();
    let handle = pool.handle();
    ready_rx.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while handle.snapshot().connected() != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut tasks = vec![];
    for _ in 0..100 {
        let h = handle.clone();
        tasks.push(tokio::spawn(async move {
            h.acquire(Duration::from_secs(1))
                .await
                .unwrap()
                .generation()
        }));
    }
    let mut generations = std::collections::HashSet::new();
    for task in tasks {
        generations.insert(task.await.unwrap());
    }
    assert_eq!(generations.len(), 2);
    assert_eq!(
        handle
            .snapshot()
            .slots
            .iter()
            .map(|s| s.attempts)
            .sum::<u64>(),
        2
    );
    pool.shutdown().await.unwrap();
    assert!(handle.snapshot().closed);
    server.await.unwrap();
}
#[tokio::test]
async fn acquisition_timeout_and_cancel_do_not_create_or_send_a_business_request() {
    let bound = TcpSocket::new_v4().unwrap();
    bound.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = bound.local_addr().unwrap();
    let pool = ConnectionPool::start(pool_config(addr, 1, 2)).unwrap();
    let handle = pool.handle();
    assert!(matches!(
        handle
            .request(begin("not-connected"), Duration::from_millis(80))
            .await,
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::PoolAcquireTimeout,
            ..
        })
    ));
    let other = handle.clone();
    let task = tokio::spawn(async move { other.acquire(Duration::from_secs(5)).await });
    tokio::task::yield_now().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(handle.snapshot().slots.len(), 1);
    assert_eq!(handle.snapshot().connected(), 0);
    pool.shutdown().await.unwrap();
}
#[tokio::test]
async fn disconnect_replaces_generation_but_never_replays_lost_begin() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let begin = read(&mut first).await;
        assert!(matches!(begin.body, Some(Message::GlobalBeginRequest(_))));
        drop(first);
        let (mut second, _) = listener.accept().await.unwrap();
        let next = read(&mut second).await;
        assert!(
            matches!(next.body, Some(Message::GlobalStatusRequest(_))),
            "pool must not resend Begin on replacement"
        );
        write(&mut second, reply(next.id, peer::status_result())).await;
        eof(&mut second).await;
    });
    let pool = ConnectionPool::start(pool_config(addr, 1, 4)).unwrap();
    let handle = pool.handle();
    let old = handle.acquire(Duration::from_secs(2)).await.unwrap();
    assert!(handle
        .request(begin("lost"), Duration::from_secs(2))
        .await
        .unwrap_err()
        .is_unknown_outcome());
    let replacement = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let new = handle.acquire(Duration::from_secs(1)).await.unwrap();
            if new.generation() != old.generation() {
                break new;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(old.snapshot().closed);
    assert_ne!(old.generation(), replacement.generation());
    assert!(matches!(
        old.request(begin("stale-handle"), Duration::from_secs(1))
            .await,
        Err(CallError::LocalFailureBeforeSend { .. })
    ));
    let response = replacement
        .request(
            Message::GlobalStatusRequest(GlobalStatusRequest {
                xid: Some("known".into()),
                extra_data: None,
            }),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert!(matches!(
        response.body,
        Some(Message::GlobalStatusResponse(_))
    ));
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn one_pool_call_dispatches_once_and_shutdown_prevents_reconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (seen_tx, seen_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read(&mut socket).await;
        seen_tx.send(()).unwrap();
        write(
            &mut socket,
            peer::heartbeat(request.id, RequestType::HeartbeatRequest),
        )
        .await;
        assert_eq!(
            read(&mut socket).await.message_type,
            RequestType::HeartbeatResponse
        );
        write(&mut socket, reply(request.id, begin_result("pooled"))).await;
        eof(&mut socket).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    });
    let pool = ConnectionPool::start(pool_config(addr, 1, 4)).unwrap();
    let handle = pool.handle();
    assert_eq!(
        xid(handle
            .request(begin("single"), Duration::from_secs(2))
            .await
            .unwrap()),
        "pooled"
    );
    seen_rx.await.unwrap();
    pool.shutdown().await.unwrap();
    assert!(matches!(
        handle.acquire(Duration::from_secs(1)).await,
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::PoolClosed,
            ..
        })
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn short_lived_connections_keep_exponential_backoff_and_stable_connection_resets_it() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (release, mut close) = tokio::sync::mpsc::channel(1);
    let server = tokio::spawn(async move {
        for _ in 0..4 {
            let (socket, _) = listener.accept().await.unwrap();
            close.recv().await.unwrap();
            drop(socket);
        }
    });
    let policy = ReconnectPolicy::new(
        Duration::from_millis(60),
        Duration::from_millis(200),
        Duration::from_millis(500),
    )
    .unwrap();
    let pool =
        ConnectionPool::start(ConnectionPoolConfig::new(addr, config(2), 1, 2, policy).unwrap())
            .unwrap();
    let handle = pool.handle();
    for (index, delay) in [60, 120, 200, 60].into_iter().enumerate() {
        let current = handle.acquire(Duration::from_secs(2)).await.unwrap();
        if index == 3 {
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
        release.send(()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), current.closed())
            .await
            .unwrap();
        let observed = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let snapshot = handle.snapshot();
                if let Some(delay) = snapshot.slots[0].reconnect_delay {
                    break (snapshot.slots[0].attempts, delay);
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(observed, (index as u64 + 1, Duration::from_millis(delay)));
    }
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn inbound_frame_preserves_origin_generation_and_saturation_closes_that_session() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (release_tx, release_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        release_rx.await.unwrap();
        for id in 20..22 {
            let mut frame = reply(id, begin("inbound"));
            frame.message_type = RequestType::RequestOneway;
            write(&mut socket, frame).await;
        }
        eof(&mut socket).await;
    });
    let mut pool = ConnectionPool::start(pool_config(addr, 1, 1)).unwrap();
    let handle = pool.handle();
    let original = handle.acquire(Duration::from_secs(2)).await.unwrap();
    release_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), original.closed())
        .await
        .unwrap();
    assert_eq!(
        original.snapshot().last_failure,
        Some(TransportError::Overloaded)
    );
    let event = pool.next_inbound().await.unwrap();
    assert_eq!(event.connection().generation(), original.generation());
    assert_eq!(event.message().id, 20);
    assert!(event.connection().snapshot().closed);
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn owner_drop_closes_active_requests_and_all_pool_handles() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (seen_tx, seen_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read(&mut socket).await;
        seen_tx.send(()).unwrap();
        eof(&mut socket).await;
    });
    let pool = ConnectionPool::start(pool_config(addr, 1, 4)).unwrap();
    let handle = pool.handle();
    let caller = handle.clone();
    let request =
        tokio::spawn(async move { caller.request(begin("drop"), Duration::from_secs(5)).await });
    seen_rx.await.unwrap();
    drop(pool);
    assert!(handle.snapshot().closed);
    assert!(request.await.unwrap().unwrap_err().is_unknown_outcome());
    assert!(handle.snapshot().closed);
    server.await.unwrap();
}

#[tokio::test]
async fn exhausted_request_id_quota_rotates_pool_generation_without_replaying_rejected_call() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut old, _) = listener.accept().await.unwrap();
        let first = read(&mut old).await;
        write(&mut old, reply(first.id, begin_result("first"))).await;
        eof(&mut old).await;
        let (mut new, _) = listener.accept().await.unwrap();
        let request = read(&mut new).await;
        assert!(
            matches!(request.body, Some(Message::GlobalStatusRequest(_))),
            "quota error must not trigger replay"
        );
        write(&mut new, reply(request.id, peer::status_result())).await;
        eof(&mut new).await;
    });
    let policy = ReconnectPolicy::new(
        Duration::from_millis(20),
        Duration::from_millis(80),
        Duration::from_secs(1),
    )
    .unwrap();
    let pool = ConnectionPool::start(
        ConnectionPoolConfig::new(addr, config(2).with_id_limits(1, 1).unwrap(), 1, 2, policy)
            .unwrap(),
    )
    .unwrap();
    let handle = pool.handle();
    let old = handle.acquire(Duration::from_secs(2)).await.unwrap();
    assert_eq!(
        xid(handle
            .request(begin("first"), Duration::from_secs(1))
            .await
            .unwrap()),
        "first"
    );
    assert!(matches!(
        handle
            .request(begin("quota-rejected"), Duration::from_secs(1))
            .await,
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::RequestIdRotationRequired { .. },
            ..
        })
    ));
    assert!(
        old.snapshot().closed,
        "exhausted generation must be withdrawn, not selected forever"
    );
    let new = handle.acquire(Duration::from_secs(2)).await.unwrap();
    assert_ne!(old.generation(), new.generation());
    assert!(matches!(
        new.request(
            Message::GlobalStatusRequest(GlobalStatusRequest {
                xid: Some("known".into()),
                extra_data: None
            }),
            Duration::from_secs(1)
        )
        .await
        .unwrap()
        .body,
        Some(Message::GlobalStatusResponse(_))
    ));
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn shutdown_cancels_connect_attempt_and_wakes_waiting_acquirer() {
    let bound = TcpSocket::new_v4().unwrap();
    bound.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let pool = ConnectionPool::start(pool_config(bound.local_addr().unwrap(), 1, 2)).unwrap();
    let handle = pool.handle();
    let waiting = handle.clone();
    let task = tokio::spawn(async move { waiting.acquire(Duration::from_secs(5)).await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while handle.snapshot().slots[0].attempts == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_millis(500), pool.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::PoolClosed,
            ..
        })
    ));
    let before = handle.snapshot();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(handle.snapshot(), before);
}
