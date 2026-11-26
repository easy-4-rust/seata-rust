//! 暂停时钟验证池等待时间计入调用预算；发送标记在此单元测试中显式模拟，不计TCP证据。
#![allow(clippy::unwrap_used)]
use super::PoolHandle;
use crate::netty::outbound_command::OutboundCommand;
use crate::netty::pending_requests::PendingRequests;
use crate::netty::pool_core::PoolCore;
use crate::netty::pool_worker_guard::PoolWorkerGuard;
use crate::{
    CallError, ConnectionConfig, ConnectionGeneration, ConnectionHandle, ConnectionPoolConfig,
    ReconnectPolicy, TransportError,
};
use seata_protocol::transaction::GlobalBeginRequest;
use seata_protocol::v1::FrameLimits;
use seata_protocol::{Message, PayloadLimits};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, Semaphore};

fn setup() -> (Arc<PoolCore>, ConnectionConfig) {
    let frames = FrameLimits::new(
        65536,
        32767,
        128,
        PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let config =
        ConnectionConfig::new(frames, 1, Duration::from_secs(1), Duration::from_secs(1)).unwrap();
    let policy = ReconnectPolicy::new(
        Duration::from_millis(10),
        Duration::from_millis(50),
        Duration::from_secs(1),
    )
    .unwrap();
    (
        Arc::new(PoolCore::new(
            ConnectionPoolConfig::new("127.0.0.1:1".parse().unwrap(), config, 1, 1, policy)
                .unwrap(),
        )),
        config,
    )
}
#[tokio::test(start_paused = true)]
async fn acquisition_time_is_part_of_one_absolute_request_budget() {
    let (core, config) = setup();
    let pool = PoolHandle {
        core: Arc::clone(&core),
    };
    let since = tokio::time::Instant::now();
    let task = tokio::spawn(async move {
        pool.request(
            Message::GlobalBeginRequest(GlobalBeginRequest {
                timeout: 30000,
                transaction_name: Some("one-budget".into()),
            }),
            Duration::from_millis(200),
        )
        .await
    });
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(80)).await;
    let (shutdown, _) = watch::channel(false);
    let pending = Arc::new(
        PendingRequests::new(ConnectionGeneration::next().unwrap(), config, shutdown).unwrap(),
    );
    let (tx, mut rx) = mpsc::channel(2);
    assert!(core.connected(
        0,
        ConnectionHandle {
            core: Arc::clone(&pending),
            outbound: tx,
            permits: Arc::new(Semaphore::new(1)),
            response_permits: Arc::new(Semaphore::new(1)),
            config
        }
    ));
    let command = rx.recv().await.unwrap();
    match command {
        OutboundCommand::Request { id, .. } => assert!(pending.mark_write_attempt(id)),
        _ => panic!("not a business request"),
    };
    tokio::time::advance(Duration::from_millis(120)).await;
    assert!(matches!(
        task.await.unwrap(),
        Err(CallError::UnknownOutcome {
            reason: TransportError::ResponseTimeout,
            ..
        })
    ));
    assert_eq!(since.elapsed(), Duration::from_millis(200));
    assert_eq!(pending.snapshot().pending, 0);
    core.stop(TransportError::PoolClosed);
}
#[tokio::test]
async fn unpolled_pool_worker_exit_stops_acquisition_and_owns_no_reconnect() {
    let (core, _) = setup();
    let handle = PoolHandle {
        core: Arc::clone(&core),
    };
    let guard = PoolWorkerGuard::new(Arc::clone(&core));
    let future = async move {
        let _guard = guard;
        std::future::pending::<()>().await;
    };
    drop(future);
    assert!(handle.snapshot().closed);
    assert!(matches!(
        handle.acquire(Duration::from_secs(1)).await,
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::PoolClosed,
            ..
        })
    ));
    assert_eq!(handle.snapshot().slots[0].attempts, 0);
}

#[tokio::test(start_paused = true)]
async fn absolute_dispatch_deadline_survives_delay_before_session_first_poll() {
    let (core, config) = setup();
    let (shutdown, _) = watch::channel(false);
    let pending = Arc::new(
        PendingRequests::new(ConnectionGeneration::next().unwrap(), config, shutdown).unwrap(),
    );
    let (tx, mut rx) = mpsc::channel(2);
    let handle = ConnectionHandle {
        core: Arc::clone(&pending),
        outbound: tx,
        permits: Arc::new(Semaphore::new(1)),
        response_permits: Arc::new(Semaphore::new(1)),
        config,
    };
    let since = tokio::time::Instant::now();
    let deadline = since + Duration::from_millis(200);
    let call = handle.request_until(
        Message::GlobalBeginRequest(GlobalBeginRequest {
            timeout: 30000,
            transaction_name: None,
        }),
        deadline,
    );
    tokio::pin!(call);
    tokio::time::advance(Duration::from_millis(80)).await;
    let command = tokio::select! {biased; result=&mut call=>panic!("unexpected completion: {result:?}"),command=rx.recv()=>command.unwrap()};
    match command {
        OutboundCommand::Request { id, .. } => assert!(pending.mark_write_attempt(id)),
        _ => panic!("not a request"),
    };
    tokio::time::advance(Duration::from_millis(120)).await;
    assert!(matches!(
        call.await,
        Err(CallError::UnknownOutcome {
            reason: TransportError::ResponseTimeout,
            ..
        })
    ));
    assert_eq!(since.elapsed(), Duration::from_millis(200));
    core.stop(TransportError::PoolClosed);
}
