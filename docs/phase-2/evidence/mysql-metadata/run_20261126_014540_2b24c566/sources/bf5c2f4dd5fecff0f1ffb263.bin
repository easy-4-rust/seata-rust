//! 暂停时钟直接运行正式登记driver；模拟写标记/对端完成，不计真实TCP证据。
#![allow(clippy::unwrap_used)]
use super::SessionRegistration;
use crate::netty::outbound_command::OutboundCommand;
use crate::netty::pending_requests::PendingRequests;
use crate::{
    ConnectionConfig, ConnectionGeneration, ConnectionHandle, RegistrationIdentity,
    RegistrationStatus, ResourceKey, ResourceRegistry, TransportError,
};
use seata_protocol::protocol::{RegisterRMResponse, RpcMessage};
use seata_protocol::v1::FrameLimits;
use seata_protocol::{
    BranchType, CompressorType, Message, PayloadLimits, RequestType, SerializerType,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, Semaphore};

#[tokio::test(start_paused = true)]
async fn initialization_steps_share_one_deadline_and_leave_later_resources_unattempted() {
    let frames = FrameLimits::new(
        65536,
        32767,
        128,
        PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let config =
        ConnectionConfig::new(frames, 4, Duration::from_secs(1), Duration::from_secs(1)).unwrap();
    let (shutdown, _) = watch::channel(false);
    let pending = Arc::new(
        PendingRequests::new(ConnectionGeneration::next().unwrap(), config, shutdown).unwrap(),
    );
    let (tx, mut rx) = mpsc::channel(8);
    let handle = ConnectionHandle {
        core: Arc::clone(&pending),
        outbound: tx,
        permits: Arc::new(Semaphore::new(4)),
        response_permits: Arc::new(Semaphore::new(4)),
        config,
    };
    let cache = ResourceRegistry::new(3, 32).unwrap();
    for name in ["db-a", "db-b", "db-c"] {
        cache
            .insert(ResourceKey::new(BranchType::At, name.into()).unwrap())
            .unwrap();
    }
    let profile = SessionRegistration::rm(
        RegistrationIdentity::new("2.8.0".into(), "app".into(), "group".into(), None).unwrap(),
        cache,
        Duration::from_millis(100),
    )
    .unwrap();
    let since = tokio::time::Instant::now();
    let task = tokio::spawn(async move { profile.initialize(&handle).await.unwrap() });
    let id = match rx.recv().await.unwrap() {
        OutboundCommand::Request { id, .. } => id,
        _ => panic!("not registration"),
    };
    assert!(pending.mark_write_attempt(id));
    tokio::time::advance(Duration::from_millis(60)).await;
    pending.complete(RpcMessage {
        id,
        message_type: RequestType::Response,
        codec: SerializerType::Seata,
        compressor: CompressorType::None,
        head_map: vec![],
        body: Some(Message::RegisterRMResponse(RegisterRMResponse {
            identified: true,
            wire_version: None,
        })),
    });
    let id = match rx.recv().await.unwrap() {
        OutboundCommand::Request { id, .. } => id,
        _ => panic!("not resource registration"),
    };
    assert!(pending.mark_write_attempt(id));
    tokio::time::advance(Duration::from_millis(40)).await;
    let report = task.await.unwrap();
    assert_eq!(since.elapsed(), Duration::from_millis(100));
    assert_eq!(report.identity.status, RegistrationStatus::Accepted);
    assert_eq!(
        report.resources[1].attempt.status,
        RegistrationStatus::UnknownOutcome
    );
    assert_eq!(
        report.resources[2].attempt.status,
        RegistrationStatus::NotAttempted
    );
    assert_eq!(pending.snapshot().pending, 0);
    assert!(rx.try_recv().is_err());
    pending.terminate(TransportError::ConnectionClosed);
}
