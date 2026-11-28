use super::pending_request::PendingRequest;
use crate::{
    CallError, ConnectionConfig, ConnectionGeneration, ConnectionStats, RequestIdAllocator,
    TransportError,
};
use seata_protocol::protocol::RpcMessage;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};
use tokio::sync::watch;
use tokio::time::Instant;

/// 单连接的同步完成仲裁；所有锁范围内仅做有界元数据/oneshot移交，不跨await。
/// 对应 Java: AbstractNettyRemoting#futures 与MessageFuture完成/超时清理职责。
pub(crate) struct PendingRequests {
    generation: ConnectionGeneration,
    inner: Mutex<PendingState>,
    shutdown: watch::Sender<bool>,
}

#[cfg(test)]
#[path = "pending_requests_tests.rs"]
mod tests;
/// PendingRequests的内部伴随状态；代际之间不共享此字典。
/// 对应 Java: AbstractNettyRemoting的futures/idGenerator局部状态组合。
struct PendingState {
    ids: RequestIdAllocator,
    pending: HashMap<i32, PendingRequest>,
    stats: ConnectionStats,
}
impl PendingRequests {
    /// 参数是独立代际、已验证预算和关闭广播；返回空的单连接等待表。
    pub(crate) fn new(
        generation: ConnectionGeneration,
        config: ConnectionConfig,
        shutdown: watch::Sender<bool>,
    ) -> Result<Self, TransportError> {
        Ok(Self {
            generation,
            inner: Mutex::new(PendingState {
                ids: RequestIdAllocator::new(config.first_id, config.id_quota)?,
                pending: HashMap::new(),
                stats: ConnectionStats::new(generation),
            }),
            shutdown,
        })
    }
    /// 返回不重复的通道身份，不依赖wire ID或端口。
    pub(crate) const fn generation(&self) -> ConnectionGeneration {
        self.generation
    }
    /// 返回此代际关闭广播的独立订阅，供池监督且不共享消费游标。
    pub(crate) fn subscribe_closed(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }
    fn state(&self) -> MutexGuard<'_, PendingState> {
        loop {
            match self.inner.lock() {
                Ok(state) => return state,
                Err(poisoned) => {
                    // panic恢复只允许转入终态，不能在不确定的旧字典上继续接收请求。
                    let mut state = poisoned.into_inner();
                    self.fail_locked(
                        &mut state,
                        TransportError::InternalState {
                            reason: "pending table lock poisoned",
                        },
                    );
                    self.inner.clear_poison();
                    drop(state);
                    self.shutdown.send_replace(true);
                }
            }
        }
    }
    /// 返回一致快照；pending按实际字典长度计算，不保存无界历史响应。
    pub(crate) fn snapshot(&self) -> ConnectionStats {
        let state = self.state();
        let mut value = state.stats.clone();
        value.pending = state.pending.len();
        value
    }
    /// 为开放的连接分配唯一ID；终态或额度耗尽时不回收旧编号。
    pub(crate) fn next_id(&self) -> Result<i32, TransportError> {
        let mut state = self.state();
        if state.stats.closed {
            return Err(TransportError::ConnectionClosed);
        }
        state.ids.allocate()
    }
    /// 在帧入队前注册完成能力；关闭、过期或重复编号时消费并丢弃该项，返回本地原因。
    pub(crate) fn register(&self, id: i32, entry: PendingRequest) -> Result<(), TransportError> {
        let mut state = self.state();
        if state.stats.closed {
            return Err(TransportError::ConnectionClosed);
        }
        if entry.deadline <= Instant::now() {
            return Err(TransportError::ResponseTimeout);
        }
        if state.pending.contains_key(&id) {
            return Err(TransportError::InternalState {
                reason: "duplicate request ID in one connection",
            });
        }
        state.pending.insert(id, entry);
        Ok(())
    }
    /// 写future首次轮询时调用；只有本方法成功后才能轮询网络写入。
    pub(crate) fn mark_write_attempt(&self, id: i32) -> bool {
        let mut state = self.state();
        if state.stats.closed {
            return false;
        }
        let Some(entry) = state.pending.get(&id) else {
            return false;
        };
        if entry.reply.is_closed() {
            if let Some(entry) = state.pending.remove(&id) {
                Self::record_cancel(&mut state, &entry);
            }
            return false;
        }
        if entry.deadline <= Instant::now() {
            if let Some(entry) = state.pending.remove(&id) {
                state.stats.timed_out_before_send =
                    state.stats.timed_out_before_send.saturating_add(1);
                entry.fail(self.generation, id, TransportError::ResponseTimeout);
            }
            return false;
        }
        entry.attempted.store(true, Ordering::Release);
        state.stats.write_attempted = state.stats.write_attempted.saturating_add(1);
        true
    }
    /// 响应与期限/取消共用一个移除者；移除和oneshot移交原子完成，响应已决定时不会再被超时覆盖。
    pub(crate) fn complete(&self, frame: RpcMessage) {
        let mut state = self.state();
        let Some(entry) = state.pending.get(&frame.id) else {
            state.stats.unmatched_responses = state.stats.unmatched_responses.saturating_add(1);
            return;
        };
        if entry.deadline <= Instant::now() {
            // 调用future可能尚未再次poll；绝对期限仍必须在响应仲裁处生效。
            if let Some(entry) = state.pending.remove(&frame.id) {
                if entry.attempted.load(Ordering::Acquire) {
                    state.stats.timed_out_after_send =
                        state.stats.timed_out_after_send.saturating_add(1);
                } else {
                    state.stats.timed_out_before_send =
                        state.stats.timed_out_before_send.saturating_add(1);
                }
                state.stats.unmatched_responses = state.stats.unmatched_responses.saturating_add(1);
                entry.fail(self.generation, frame.id, TransportError::ResponseTimeout);
            }
            return;
        }
        let reason = if !entry.attempted.load(Ordering::Acquire) {
            Some("response arrived before any write attempt")
        } else if !frame
            .body
            .as_ref()
            .is_some_and(|body| entry.expected.matches(body))
        {
            Some("response type or flat merge shape differs")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.fail_locked(&mut state, TransportError::UnexpectedResponse { reason });
            drop(state);
            self.shutdown.send_replace(true);
            return;
        }
        if let Some(entry) = state.pending.remove(&frame.id) {
            state.stats.completed = state.stats.completed.saturating_add(1);
            // send不await；必须在同一个锁边界内移交，不能留下“已remove、尚未send”的期限竞态。
            let _ = entry.reply.send(Ok(frame));
        }
    }
    /// 调用方期限分支尝试取得唯一完成权；None表示其他事件已在同一锁边界完成oneshot。
    pub(crate) fn expire(&self, id: i32) -> Option<CallError> {
        let mut state = self.state();
        let entry = state.pending.remove(&id)?;
        let attempted = entry.attempted.load(Ordering::Acquire);
        if attempted {
            state.stats.timed_out_after_send = state.stats.timed_out_after_send.saturating_add(1);
        } else {
            state.stats.timed_out_before_send = state.stats.timed_out_before_send.saturating_add(1);
        }
        Some(CallError::request(
            self.generation,
            id,
            attempted,
            TransportError::ResponseTimeout,
        ))
    }
    /// future取消时同步移除等待项；已完成的ID不会再次计数或重新插入。
    pub(crate) fn cancel(&self, id: i32) {
        let mut state = self.state();
        if let Some(entry) = state.pending.remove(&id) {
            Self::record_cancel(&mut state, &entry);
        }
    }
    fn record_cancel(state: &mut PendingState, entry: &PendingRequest) {
        if entry.attempted.load(Ordering::Acquire) {
            state.stats.cancelled_after_send = state.stats.cancelled_after_send.saturating_add(1);
        } else {
            state.stats.cancelled_before_send = state.stats.cancelled_before_send.saturating_add(1);
        }
    }
    /// 心跳只记录控制消息，不访问普通请求的完成能力。
    pub(crate) fn heartbeat(&self) {
        let mut state = self.state();
        state.stats.heartbeats_received = state.stats.heartbeats_received.saturating_add(1);
    }
    /// 参数为同步回包仲裁操作；在代际关闭锁内执行并返回其结果，不允许持锁跨await或重入本状态。
    pub(crate) fn with_response_state<R>(
        &self,
        operation: impl FnOnce(&mut ConnectionStats) -> R,
    ) -> R {
        operation(&mut self.state().stats)
    }
    /// 参数为true表示已完整写入PING，false表示PONG；只更新开放连接的控制计数。
    pub(crate) fn heartbeat_written(&self, ping: bool) {
        let mut state = self.state();
        if state.stats.closed {
            return;
        }
        if ping {
            state.stats.heartbeat_requests_sent =
                state.stats.heartbeat_requests_sent.saturating_add(1);
        } else {
            state.stats.heartbeat_responses_sent =
                state.stats.heartbeat_responses_sent.saturating_add(1);
        }
    }
    /// 记录成功交付给有界入站队列的请求。
    pub(crate) fn inbound(&self) {
        let mut state = self.state();
        state.stats.inbound_requests = state.stats.inbound_requests.saturating_add(1);
    }
    /// 永久终止该连接、按每个写入标记清空等待表并唤醒所有后台任务；重复终止不改写首次原因。
    pub(crate) fn terminate(&self, reason: TransportError) {
        let mut state = self.state();
        self.fail_locked(&mut state, reason);
        drop(state);
        self.shutdown.send_replace(true);
    }
    fn fail_locked(&self, state: &mut PendingState, reason: TransportError) {
        if !state.stats.closed {
            if reason == TransportError::ReadIdleTimeout {
                state.stats.read_idle_expirations =
                    state.stats.read_idle_expirations.saturating_add(1);
            }
            state.stats.closed = true;
            state.stats.last_failure = Some(reason.clone());
        }
        let reason = state.stats.last_failure.as_ref().cloned().unwrap_or(reason);
        for (id, entry) in state.pending.drain() {
            entry.fail(self.generation, id, reason.clone());
        }
    }
}
