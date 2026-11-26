use super::pending_requests::PendingRequests;
use crate::{CallError, ConnectionStats, TransportError};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio::time::Instant;
/// 单个响应帧的本地写完成仲裁，与请求future表及TC wire ID命名空间独立。
/// 对应 Java: io.netty.channel.ChannelFuture的写入完成职责；Rust另保留写前/未知分类。
pub(crate) struct WriteReceipt {
    pub(crate) core: Arc<PendingRequests>,
    id: i32,
    deadline: Instant,
    inner: Mutex<ReceiptState>,
}

/// WriteReceipt内部伴随状态；只允许一个完成者，不跨await持锁。
/// 对应 Java: ChannelPromise的单次完成状态。
struct ReceiptState {
    attempted: bool,
    reply: Option<oneshot::Sender<Result<(), CallError>>>,
}
impl WriteReceipt {
    /// 参数为原代际、原wire ID、绝对期限及完成通道；返回尚未开始写入的独立回执。
    pub(crate) fn new(
        core: Arc<PendingRequests>,
        id: i32,
        deadline: Instant,
        reply: oneshot::Sender<Result<(), CallError>>,
    ) -> Self {
        Self {
            core,
            id,
            deadline,
            inner: Mutex::new(ReceiptState {
                attempted: false,
                reply: Some(reply),
            }),
        }
    }
    fn arbitrate<R>(
        &self,
        operation: impl FnOnce(&mut ReceiptState, &mut ConnectionStats) -> R,
    ) -> R {
        let (value, poisoned) = self.core.with_response_state(|stats| {
            // 固定锁顺序：代际状态→回执；关闭与成功移交共享同一代际锁，均不跨await。
            let (mut state, poisoned) = match self.inner.lock() {
                Ok(state) => (state, false),
                Err(poisoned) => {
                    let mut state = poisoned.into_inner();
                    state.attempted = true;
                    self.fail_locked(
                        &mut state,
                        TransportError::InternalState {
                            reason: "response receipt lock poisoned",
                        },
                    );
                    self.inner.clear_poison();
                    (state, true)
                }
            };
            (operation(&mut state, stats), poisoned)
        });
        // 毒锁终止放在外部，不能在持有代际锁时重入terminate。
        if poisoned {
            self.core.terminate(TransportError::InternalState {
                reason: "response receipt lock poisoned",
            });
        }
        value
    }
    fn error(&self, state: &ReceiptState, reason: TransportError) -> CallError {
        CallError::request(self.core.generation(), self.id, state.attempted, reason)
    }
    fn fail_locked(&self, state: &mut ReceiptState, reason: TransportError) -> Option<CallError> {
        let sender = state.reply.take()?;
        let error = self.error(state, reason);
        let _ = sender.send(Err(error.clone()));
        Some(error)
    }
    /// 写future首次轮询时调用；过期/已取消/已关闭则跳过整帧，不能误标已发送。
    pub(crate) fn begin(&self) -> bool {
        self.arbitrate(|state, stats| {
            if stats.closed {
                self.fail_locked(
                    state,
                    stats
                        .last_failure
                        .clone()
                        .unwrap_or(TransportError::ConnectionClosed),
                );
                return false;
            }
            let Some(sender) = &state.reply else {
                return false;
            };
            if sender.is_closed() {
                state.reply = None;
                return false;
            }
            if self.deadline <= Instant::now() {
                self.fail_locked(state, TransportError::ResponseTimeout);
                return false;
            }
            state.attempted = true;
            stats.response_write_attempted = stats.response_write_attempted.saturating_add(1);
            true
        })
    }
    /// 完整帧本地写完时调用；仅期限前完成返回Ok，不代表TC已收到或业务已持久完成。
    pub(crate) fn written(&self) {
        self.arbitrate(|state, stats| {
            if stats.closed {
                self.fail_locked(
                    state,
                    stats
                        .last_failure
                        .clone()
                        .unwrap_or(TransportError::ConnectionClosed),
                );
                return;
            }
            stats.responses_written = stats.responses_written.saturating_add(1);
            if self.deadline <= Instant::now() {
                self.fail_locked(state, TransportError::ResponseTimeout);
                return;
            }
            if let Some(sender) = state.reply.take() {
                let _ = sender.send(Ok(()));
            }
        });
    }
    /// 超时/关闭/任务释放竞争单一完成权，返回取得完成权的错误；None表示其他完成者已移交。
    pub(crate) fn fail(&self, reason: TransportError) -> Option<CallError> {
        self.arbitrate(|state, stats| {
            self.fail_locked(
                state,
                if stats.closed {
                    stats.last_failure.clone().unwrap_or(reason)
                } else {
                    reason
                },
            )
        })
    }
    /// 取得当前代际的首次关闭原因；没有更具体原因时使用ConnectionClosed。
    pub(crate) fn close_reason(&self) -> TransportError {
        self.core
            .snapshot()
            .last_failure
            .unwrap_or(TransportError::ConnectionClosed)
    }
    /// 完成通道异常消失时关闭代际，保留原写入标记，避免错误宣称未发送。
    pub(crate) fn lost_completion(&self) -> CallError {
        let reason = TransportError::InternalState {
            reason: "response write completion disappeared",
        };
        let error = self.arbitrate(|state, _| self.error(state, reason.clone()));
        self.core.terminate(reason);
        error
    }
}

#[cfg(test)]
mod tests {
    use super::WriteReceipt;
    use crate::netty::pending_requests::PendingRequests;
    use crate::{CallError, ConnectionConfig, ConnectionGeneration, TransportError};
    use seata_protocol::v1::FrameLimits;
    use seata_protocol::PayloadLimits;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{oneshot, watch};
    #[tokio::test]
    async fn closure_and_full_write_arbitrate_before_unpolled_caller_observes_result() {
        for close_first in [false, true] {
            let frames = FrameLimits::new(
                65536,
                32767,
                128,
                PayloadLimits::new(65536, 512, 1024, 16).unwrap_or_else(|error| panic!("{error}")),
            )
            .unwrap_or_else(|error| panic!("{error}"));
            let config =
                ConnectionConfig::new(frames, 1, Duration::from_secs(1), Duration::from_secs(1))
                    .unwrap_or_else(|error| panic!("{error}"));
            let (shutdown, _) = watch::channel(false);
            let core = Arc::new(
                PendingRequests::new(
                    ConnectionGeneration::next().unwrap_or_else(|error| panic!("{error}")),
                    config,
                    shutdown,
                )
                .unwrap_or_else(|error| panic!("{error}")),
            );
            let (sender, mut receiver) = oneshot::channel();
            let receipt = WriteReceipt::new(
                Arc::clone(&core),
                -1,
                tokio::time::Instant::now() + Duration::from_secs(1),
                sender,
            );
            assert!(receipt.begin());
            if close_first {
                core.terminate(TransportError::ReadIdleTimeout);
            }
            receipt.written();
            if !close_first {
                core.terminate(TransportError::ReadIdleTimeout);
            }
            let result = receiver
                .try_recv()
                .unwrap_or_else(|error| panic!("{error}"));
            if close_first {
                assert!(
                    matches!(
                        result,
                        Err(CallError::UnknownOutcome {
                            reason: TransportError::ReadIdleTimeout,
                            ..
                        })
                    ),
                    "closed generation published write success to unpolled caller: {result:?}"
                );
            } else {
                assert_eq!(result, Ok(()));
            }
        }
    }
}
