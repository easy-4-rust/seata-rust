//! WP07 应用事务准入、作用域持有与停机排空验收。
#![allow(clippy::unwrap_used)]
use seata_tm::{AdmissionError, TransactionAdmission};
use std::{future::Future, time::Duration};
use tokio::sync::oneshot;

#[test]
fn configuration_capacity_and_shared_close() {
    assert!(matches!(
        TransactionAdmission::new(0),
        Err(AdmissionError::InvalidCapacity)
    ));
    assert!(matches!(
        TransactionAdmission::new(usize::MAX),
        Err(AdmissionError::InvalidCapacity)
    ));
    let gate = TransactionAdmission::new(1).unwrap();
    let peer = gate.clone();
    let permit = gate.try_enter().unwrap();
    assert!(matches!(peer.try_enter(), Err(AdmissionError::AtCapacity)));
    drop(permit);
    let permit = peer.try_enter().unwrap();
    gate.close();
    assert!(matches!(peer.try_enter(), Err(AdmissionError::Closed)));
    assert_eq!(gate.active(), 1);
    drop(permit);
    assert_eq!(gate.active(), 0);
}

#[tokio::test(start_paused = true)]
async fn drain_times_out_without_cancelling_scope_and_can_be_retried() {
    let gate = TransactionAdmission::new(2).unwrap();
    let permit = gate.try_enter().unwrap();
    assert_eq!(
        gate.drain(Duration::from_secs(5)).await,
        Err(AdmissionError::DrainTimeout { active: 1 })
    );
    assert!(matches!(gate.try_enter(), Err(AdmissionError::Closed)));
    assert_eq!(gate.active(), 1);
    drop(permit);
    gate.drain(Duration::ZERO).await.unwrap();
}

#[tokio::test]
async fn multiple_drainers_wait_until_the_last_admitted_scope_exits() {
    let gate = TransactionAdmission::new(2).unwrap();
    let first = gate.try_enter().unwrap();
    let second = gate.try_enter().unwrap();
    let mut workers = Vec::new();
    for _ in 0..2 {
        let gate = gate.clone();
        let (started, ready) = oneshot::channel();
        workers.push(tokio::spawn(async move {
            let mut drain = Box::pin(gate.drain(Duration::from_secs(2)));
            let mut started = Some(started);
            std::future::poll_fn(|cx| {
                let result = drain.as_mut().poll(cx);
                if result.is_pending() {
                    if let Some(started) = started.take() {
                        started.send(()).unwrap();
                    }
                }
                result
            })
            .await
        }));
        ready.await.unwrap();
    }
    drop(first);
    tokio::task::yield_now().await;
    assert!(workers.iter().all(|worker| !worker.is_finished()));
    drop(second);
    for worker in workers {
        tokio::time::timeout(Duration::from_secs(1), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

fn futures_poll<F: std::future::Future + Unpin>(future: &mut F) -> std::task::Poll<F::Output> {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    std::pin::Pin::new(future).poll(&mut context)
}

#[tokio::test]
async fn cancelling_drain_keeps_gate_closed_and_scope_owned() {
    let gate = TransactionAdmission::new(1).unwrap();
    let permit = gate.try_enter().unwrap();
    let mut drain = Box::pin(gate.drain(Duration::from_secs(2)));
    assert!(matches!(futures_poll(&mut drain), std::task::Poll::Pending));
    drop(drain);
    assert!(matches!(gate.try_enter(), Err(AdmissionError::Closed)));
    assert_eq!(gate.active(), 1);
    drop(permit);
    gate.drain(Duration::ZERO).await.unwrap();
}

#[tokio::test]
async fn cancelled_and_panicking_workers_release_their_scopes() {
    let gate = TransactionAdmission::new(1).unwrap();
    let permit = gate.try_enter().unwrap();
    let (started, ready) = oneshot::channel();
    let worker = tokio::spawn(async move {
        let _permit = permit;
        started.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    ready.await.unwrap();
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    let permit = gate.try_enter().unwrap();
    assert!(tokio::spawn(async move {
        let _permit = permit;
        panic!("business failure");
    })
    .await
    .unwrap_err()
    .is_panic());
    gate.drain(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn close_racing_with_admission_never_reopens_the_gate() {
    let gate = TransactionAdmission::new(32).unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(33));
    let mut workers = Vec::new();
    for _ in 0..32 {
        let gate = gate.clone();
        let barrier = barrier.clone();
        workers.push(tokio::spawn(async move {
            barrier.wait().await;
            match gate.try_enter() {
                Ok(permit) => {
                    tokio::task::yield_now().await;
                    drop(permit);
                }
                Err(AdmissionError::Closed) => {}
                other => panic!("unexpected admission outcome: {other:?}"),
            }
        }));
    }
    barrier.wait().await;
    gate.close();
    for worker in workers {
        worker.await.unwrap();
    }
    gate.drain(Duration::from_secs(2)).await.unwrap();
    assert_eq!(gate.active(), 0);
    assert!(matches!(gate.try_enter(), Err(AdmissionError::Closed)));
}

#[path = "support/tm_peer.rs"]
mod fixture;

#[tokio::test]
async fn admitted_native_tm_scope_finishes_before_owner_shutdown() {
    use fixture::{begin, config, end_response, manager, peer, registered};
    use seata_protocol::{GlobalStatus, Message};
    use tokio::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let gate = TransactionAdmission::new(1).unwrap();
    let permit = gate.try_enter().unwrap();
    let (commit_seen, seen) = oneshot::channel();
    let (release, proceed) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:shutdown-scope")).await;
        let request = peer::read(&mut socket).await;
        match request.body {
            Some(Message::GlobalCommitRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("tc:shutdown-scope"));
            }
            other => panic!("unexpected request: {other:?}"),
        }
        commit_seen.send(()).unwrap();
        proceed.await.unwrap();
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                end_response(true, GlobalStatus::AsyncCommitting, peer::success()),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let business = tokio::spawn(async move {
        let _permit = permit;
        let mut tx = tm.begin(&config()).await.unwrap();
        let result = tx.commit().await.unwrap();
        assert!(matches!(
            result,
            seata_tm::TransactionCompletion::Observed { .. }
        ));
        drop(tx);
        tm
    });
    seen.await.unwrap();
    assert_eq!(
        gate.drain(Duration::from_millis(20)).await,
        Err(AdmissionError::DrainTimeout { active: 1 })
    );
    assert!(matches!(gate.try_enter(), Err(AdmissionError::Closed)));
    release.send(()).unwrap();
    let tm = business.await.unwrap();
    gate.drain(Duration::from_secs(1)).await.unwrap();
    tm.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
}
