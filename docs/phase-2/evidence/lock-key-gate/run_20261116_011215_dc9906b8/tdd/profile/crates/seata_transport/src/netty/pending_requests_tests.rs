//! 等待表的确定性完成边界；不借助网络调度偶然制造期限竞态。
#![allow(clippy::unwrap_used)]
use super::PendingRequests;
use crate::netty::pending_request::PendingRequest;
use crate::netty::response_expectation::ResponseExpectation;
use crate::netty::worker_exit_guard::WorkerExitGuard;
use crate::{CallError, ConnectionConfig, ConnectionGeneration, TransportError};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::{AbstractTransactionResponse, GlobalBeginResponse};
use seata_protocol::v1::FrameLimits;
use seata_protocol::{
    CompressorType, Message, MessageType, PayloadLimits, RequestType, ResultCode, SerializerType,
    TransactionExceptionCode,
};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, watch, Semaphore};
use tokio::time::Instant;

#[tokio::test]
async fn synchronous_pending_lock_wait_is_included_in_request_deadline() {
    use crate::ConnectionHandle;
    use seata_protocol::transaction::GlobalBeginRequest;
    use tokio::sync::mpsc;
    let core = core();
    let frames = FrameLimits::new(
        65536,
        32767,
        128,
        PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let config =
        ConnectionConfig::new(frames, 1, Duration::from_secs(1), Duration::from_secs(1)).unwrap();
    let (tx, mut rx) = mpsc::channel(2);
    let handle = ConnectionHandle {
        core: Arc::clone(&core),
        outbound: tx,
        permits: Arc::new(Semaphore::new(1)),
        response_permits: Arc::new(Semaphore::new(1)),
        config,
    };
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let other = Arc::clone(&core);
    let held = Arc::clone(&barrier);
    let owner = std::thread::spawn(move || {
        let _lock = other.inner.lock().unwrap();
        held.wait();
        std::thread::sleep(Duration::from_millis(60));
    });
    barrier.wait();
    {
        let request = handle.request(
            Message::GlobalBeginRequest(GlobalBeginRequest {
                timeout: 30000,
                transaction_name: Some("lock-budget".into()),
            }),
            Duration::from_secs(5),
        );
        tokio::pin!(request);
        let command = tokio::select! {biased; result=&mut request=>panic!("unexpected completion: {result:?}"),command=rx.recv()=>command.unwrap()};
        let id = match command {
            crate::netty::outbound_command::OutboundCommand::Request { id, .. } => id,
            _ => panic!("not a request"),
        };
        let remaining = core.inner.lock().unwrap().pending[&id]
            .deadline
            .saturating_duration_since(Instant::now());
        assert!(
            remaining < Duration::from_millis(4980),
            "lock wait incorrectly restarted request budget: {remaining:?}"
        );
    }
    owner.join().unwrap();
    assert_eq!(core.snapshot().pending, 0);
}

fn core() -> Arc<PendingRequests> {
    let frames = FrameLimits::new(
        65536,
        32767,
        128,
        PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let config =
        ConnectionConfig::new(frames, 2, Duration::from_secs(1), Duration::from_secs(1)).unwrap();
    let (shutdown, _) = watch::channel(false);
    Arc::new(PendingRequests::new(ConnectionGeneration::next().unwrap(), config, shutdown).unwrap())
}
fn entry(
    core: &PendingRequests,
    deadline: Instant,
) -> (i32, oneshot::Receiver<Result<RpcMessage, CallError>>) {
    let id = core.next_id().unwrap();
    let (reply, receiver) = oneshot::channel();
    core.register(
        id,
        PendingRequest {
            expected: ResponseExpectation::Single(MessageType::GlobalBeginResponse),
            deadline,
            reply,
            attempted: Arc::new(AtomicBool::new(false)),
            _permit: Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap(),
        },
    )
    .unwrap();
    (id, receiver)
}
fn response(id: i32) -> RpcMessage {
    RpcMessage {
        id,
        message_type: RequestType::Response,
        codec: SerializerType::Seata,
        compressor: CompressorType::None,
        head_map: vec![],
        body: Some(Message::GlobalBeginResponse(GlobalBeginResponse {
            response: AbstractTransactionResponse {
                result_code: ResultCode::Success,
                msg: None,
                transaction_exception_code: TransactionExceptionCode::Unknown,
            },
            xid: Some("winner".into()),
            extra_data: None,
        })),
    }
}

#[tokio::test]
async fn queued_expiry_is_local_and_never_reaches_write_marker() {
    let core = core();
    let (id, mut receiver) = entry(&core, Instant::now() + Duration::from_secs(1));
    let error = core.expire(id).unwrap();
    assert!(matches!(
        error,
        CallError::LocalFailureBeforeSend {
            reason: TransportError::ResponseTimeout,
            ..
        }
    ));
    assert!(!core.mark_write_attempt(id));
    assert!(receiver.try_recv().is_err());
    assert_eq!(core.snapshot().timed_out_before_send, 1);
    assert_eq!(core.snapshot().write_attempted, 0);
    assert_eq!(core.snapshot().pending, 0);
}
#[tokio::test]
async fn writer_skips_queued_frame_whose_deadline_already_elapsed() {
    let core = core();
    let (id, mut receiver) = entry(&core, Instant::now() + Duration::from_millis(1));
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!core.mark_write_attempt(id));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::ResponseTimeout,
            ..
        })
    ));
    assert_eq!(core.snapshot().timed_out_before_send, 1);
    assert_eq!(core.snapshot().pending, 0);
}
#[tokio::test]
async fn completed_response_is_available_before_losing_deadline_branch_can_observe_removal() {
    let core = core();
    let (id, mut receiver) = entry(&core, Instant::now() + Duration::from_secs(1));
    assert!(core.mark_write_attempt(id));
    let expected = response(id);
    core.complete(expected.clone());
    assert!(core.expire(id).is_none());
    assert_eq!(receiver.try_recv().unwrap().unwrap(), expected);
    core.cancel(id);
    core.complete(response(id));
    assert_eq!(core.snapshot().completed, 1);
    assert_eq!(core.snapshot().unmatched_responses, 1);
    assert_eq!(core.snapshot().timed_out_after_send, 0);
    assert_eq!(core.snapshot().cancelled_after_send, 0);
}
#[tokio::test]
async fn close_classifies_each_pending_by_its_own_write_marker() {
    let core = core();
    let (queued, mut local) = entry(&core, Instant::now() + Duration::from_secs(1));
    let (sent, mut unknown) = entry(&core, Instant::now() + Duration::from_secs(1));
    assert!(core.mark_write_attempt(sent));
    core.terminate(TransportError::ConnectionClosed);
    assert!(
        matches!(local.try_recv().unwrap(),Err(CallError::LocalFailureBeforeSend {request_id:Some(id),..}) if id==queued)
    );
    assert!(
        matches!(unknown.try_recv().unwrap(),Err(CallError::UnknownOutcome {request_id,..}) if request_id==sent)
    );
    assert_eq!(core.snapshot().pending, 0);
    assert!(!core.mark_write_attempt(queued));
}

#[tokio::test]
async fn response_after_absolute_deadline_is_late_even_if_caller_timer_was_not_polled() {
    let core = core();
    let (id, mut receiver) = entry(&core, Instant::now() + Duration::from_millis(1));
    assert!(core.mark_write_attempt(id));
    // receiver一直未poll，期限必须由接收仲裁检查，不能依赖调用者被调度。
    tokio::time::sleep(Duration::from_millis(10)).await;
    core.complete(response(id));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        Err(CallError::UnknownOutcome {
            reason: TransportError::ResponseTimeout,
            ..
        })
    ));
    assert_eq!(core.snapshot().completed, 0);
    assert_eq!(core.snapshot().timed_out_after_send, 1);
    assert_eq!(core.snapshot().unmatched_responses, 1);
    assert_eq!(core.snapshot().pending, 0);
}
#[tokio::test]
async fn response_before_first_write_closes_connection_without_claiming_business_rejection() {
    let core = core();
    let (id, mut receiver) = entry(&core, Instant::now() + Duration::from_secs(1));
    core.complete(response(id));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::UnexpectedResponse { .. },
            ..
        })
    ));
    assert!(core.snapshot().closed);
    assert_eq!(core.snapshot().completed, 0);
}
#[tokio::test]
async fn unpolled_worker_guard_and_poisoned_lock_both_clear_pending() {
    let core = core();
    let (id, mut receiver) = entry(&core, Instant::now() + Duration::from_secs(1));
    assert!(core.mark_write_attempt(id));
    let guard = WorkerExitGuard::new(Arc::clone(&core));
    let unpolled = async move {
        let _guard = guard;
        std::future::pending::<()>().await;
    };
    drop(unpolled);
    assert!(receiver
        .try_recv()
        .unwrap()
        .unwrap_err()
        .is_unknown_outcome());
    assert!(core.snapshot().closed);

    let poisoned = super::tests::core();
    let (id, mut receiver) = entry(&poisoned, Instant::now() + Duration::from_secs(1));
    assert!(poisoned.mark_write_attempt(id));
    let other = Arc::clone(&poisoned);
    assert!(std::thread::spawn(move || {
        let _lock = other.inner.lock().unwrap();
        panic!("controlled poison");
    })
    .join()
    .is_err());
    assert!(poisoned.snapshot().closed);
    assert_eq!(poisoned.snapshot().pending, 0);
    assert!(matches!(
        receiver.try_recv().unwrap(),
        Err(CallError::UnknownOutcome {
            reason: TransportError::InternalState { .. },
            ..
        })
    ));
}
