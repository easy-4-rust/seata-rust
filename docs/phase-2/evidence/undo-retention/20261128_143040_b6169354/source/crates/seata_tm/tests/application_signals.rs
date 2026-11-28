//! 独立进程 OS 信号与原生 TM 作用域排空联试；由受控运行器触发真实事件。
#![cfg(all(feature = "application_signals", any(windows, unix)))]
#![allow(clippy::unwrap_used)]
use seata_tm::{AdmissionError, ApplicationSignals, ShutdownSignalKind, TransactionAdmission};
use std::{path::PathBuf, time::Duration};
use tokio::sync::oneshot;
#[path = "support/tm_peer.rs"]
mod fixture;

fn save(folder: &std::path::Path, name: &str, body: &str) {
    std::fs::write(folder.join(name), body).unwrap();
}

#[tokio::test]
#[ignore = "requires an isolated console and an externally delivered OS signal"]
async fn real_os_signal_drains_native_tm_scope() {
    use fixture::{begin, config, end_response, manager, peer, registered};
    use seata_protocol::{GlobalStatus, Message};
    use tokio::net::TcpListener;

    let folder = PathBuf::from(std::env::var("SEATA_SIGNAL_OUT").unwrap());
    let mut signals = ApplicationSignals::install().unwrap();
    // 取消一次等待不会丢失接收器，也不会更改应用准入。
    let mut wait = Box::pin(signals.recv());
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(wait.as_mut(), &mut context).is_pending());
    drop(wait);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(10));
    let sampler = tm.connection_sampler();
    let gate = TransactionAdmission::new(2).unwrap();
    let permit = gate.try_enter().unwrap();
    let (seen_tx, seen) = oneshot::channel();
    let (release, proceed) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:os-signal-scope")).await;
        let request = peer::read(&mut socket).await;
        match request.body {
            Some(Message::GlobalCommitRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("tc:os-signal-scope"));
            }
            other => panic!("unexpected request: {other:?}"),
        }
        seen_tx.send(()).unwrap();
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
        match result {
            seata_tm::TransactionCompletion::Observed { outcome, .. } => {
                assert_eq!(outcome.status(), GlobalStatus::AsyncCommitting);
            }
            other => panic!("unexpected transaction completion: {other:?}"),
        }
        drop(tx);
        tm
    });
    seen.await.unwrap();
    drop(gate.try_enter().unwrap());
    save(
        &folder,
        "ready.json",
        &format!(
            r#"{{"pid":{},"active":1,"commit_received":true}}"#,
            std::process::id()
        ),
    );
    let kind = tokio::time::timeout(Duration::from_secs(20), signals.recv())
        .await
        .unwrap()
        .unwrap();
    let expected = match std::env::var("SEATA_EXPECTED_SIGNAL").unwrap().as_str() {
        "interrupt" => ShutdownSignalKind::Interrupt,
        "console_break" => ShutdownSignalKind::ConsoleBreak,
        "terminate" => ShutdownSignalKind::Terminate,
        other => panic!("unsupported expected signal: {other}"),
    };
    assert_eq!(kind, expected);
    gate.close();
    assert!(matches!(gate.try_enter(), Err(AdmissionError::Closed)));
    assert_eq!(
        gate.drain(Duration::from_millis(30)).await,
        Err(AdmissionError::DrainTimeout { active: 1 })
    );
    assert!(!sampler.snapshot().closed);
    assert!(!business.is_finished());
    save(
        &folder,
        "received.json",
        r#"{"signal_matched":true,"admission_closed":true,"active":1,"owner_open":true}"#,
    );
    tokio::time::timeout(Duration::from_secs(20), async {
        while !folder.join("release").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let repeated = tokio::time::timeout(Duration::from_secs(2), signals.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(repeated, expected);
    release.send(()).unwrap();
    let tm = business.await.unwrap();
    gate.drain(Duration::from_secs(1)).await.unwrap();
    tm.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    assert!(sampler.snapshot().closed);
    save(
        &folder,
        "completed.json",
        r#"{"commit_requests":1,"active":0,"owner_closed":true,"peer_eof":true,"repeated_signal":true}"#,
    );
}
