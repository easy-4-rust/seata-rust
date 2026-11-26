use crate::{
    ConnectionHandle, ConnectionPoolConfig, PoolCounters, PoolSlotSnapshot, PoolSlotState,
    PoolSnapshot, SessionRegistration, SessionRegistrationReport, TransportError,
};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::watch;
/// raw池的固定槽位选择与生命周期仲裁，无锁跨await；业务请求不进入此状态机重试。
/// 对应 Java: NettyClientChannelManager的通道表与池级关闭职责。
pub(crate) struct PoolCore {
    pub(crate) config: ConnectionPoolConfig,
    pub(crate) registration: Option<SessionRegistration>,
    #[cfg(feature = "tls")]
    pub(crate) tls: Option<crate::TlsClientConfig>,
    inner: Mutex<PoolState>,
    changed: watch::Sender<u64>,
    shutdown: watch::Sender<bool>,
}
/// PoolCore内部伴随状态；仅保存每槽位当前句柄和有界统计。
/// 对应 Java: NettyClientChannelManager通道缓存。
struct PoolState {
    snapshot: PoolSnapshot,
    handles: Vec<Option<ConnectionHandle>>,
    retiring: Vec<Option<ConnectionHandle>>,
    next: usize,
}
impl PoolCore {
    /// 参数是已验证配置；返回固定大小槽位及独立事件广播。
    pub(crate) fn new(config: ConnectionPoolConfig) -> Self {
        Self::create(config, None)
    }
    /// 参数为池配置及原生登记策略，返回尚未建立任何ready代际的状态。
    pub(crate) fn with_registration(
        config: ConnectionPoolConfig,
        registration: SessionRegistration,
    ) -> Self {
        Self::create(config, Some(registration))
    }
    fn create(config: ConnectionPoolConfig, registration: Option<SessionRegistration>) -> Self {
        let (changed, _) = watch::channel(0);
        let (shutdown, _) = watch::channel(false);
        Self {
            config,
            registration,
            #[cfg(feature = "tls")]
            tls: None,
            inner: Mutex::new(PoolState {
                snapshot: PoolSnapshot {
                    closed: false,
                    last_failure: None,
                    slots: (0..config.slots)
                        .map(|_| PoolSlotSnapshot {
                            state: PoolSlotState::Connecting,
                            generation: None,
                            attempts: 0,
                            last_failure: None,
                            reconnect_delay: None,
                            registration: None,
                            connection: None,
                        })
                        .collect(),
                    cumulative: PoolCounters::default(),
                },
                handles: vec![None; config.slots],
                retiring: vec![None; config.slots],
                next: 0,
            }),
            changed,
            shutdown,
        }
    }
    fn state(&self) -> MutexGuard<'_, PoolState> {
        loop {
            match self.inner.lock() {
                Ok(state) => return state,
                Err(poisoned) => {
                    let mut state = poisoned.into_inner();
                    Self::stop_locked(
                        &mut state,
                        TransportError::InternalState {
                            reason: "pool state lock poisoned",
                        },
                    );
                    self.inner.clear_poison();
                    drop(state);
                    self.shutdown.send_replace(true);
                    self.notify();
                }
            }
        }
    }
    fn notify(&self) {
        self.changed
            .send_modify(|value| *value = value.saturating_add(1));
    }
    /// 返回有界池状态快照，连接计数在池锁外按同代际采样，非跨锁原子瞬时值。
    pub(crate) fn snapshot(&self) -> PoolSnapshot {
        let (mut snapshot, handles, retiring) = {
            let state = self.state();
            (
                state.snapshot.clone(),
                state.handles.clone(),
                state.retiring.clone(),
            )
        };
        // 在池锁外读取各连接的独立统计；只把同代际计数附到本次槽位视图。
        for (slot, handle) in snapshot.slots.iter_mut().zip(&handles) {
            if let Some(handle) = handle {
                let current = handle.snapshot();
                if !snapshot.closed && slot.generation == Some(current.generation) {
                    slot.connection = Some(current);
                }
            }
        }
        // 池锁内复制的归档基线与两个有界句柄表属于同一次状态视图，避免撤回竞态漏计或重计。
        for handle in handles.into_iter().chain(retiring).flatten() {
            snapshot
                .cumulative
                .accumulate(PoolCounters::from_connection(&handle.snapshot()));
        }
        if let Some(registration) = &self.registration {
            let current = registration.revision();
            for slot in &mut snapshot.slots {
                if slot.state == PoolSlotState::Ready
                    && !slot.registration.as_ref().is_some_and(|report| {
                        current
                            .as_ref()
                            .is_ok_and(|revision| *revision == report.registry_revision)
                    })
                {
                    slot.state = if current.is_ok() {
                        PoolSlotState::Registering
                    } else {
                        PoolSlotState::Degraded
                    };
                }
            }
        }
        snapshot
    }
    /// 返回状态变化订阅；先订阅再检查状态以避免错过开放/关闭事件。
    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }
    /// 返回独立关闭订阅，供每个固定槽位监督器使用。
    pub(crate) fn stop_receiver(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }
    /// 选择开放raw通道；轮转只移动选择游标，不迁移旧句柄或隐式发送请求。
    pub(crate) fn choose(&self) -> Result<Option<ConnectionHandle>, TransportError> {
        let mut state = self.state();
        if state.snapshot.closed {
            return Err(TransportError::PoolClosed);
        }
        let revision = self
            .registration
            .as_ref()
            .map(SessionRegistration::revision)
            .transpose()?;
        for offset in 0..state.handles.len() {
            let index = (state.next + offset) % state.handles.len();
            let slot = &state.snapshot.slots[index];
            let selectable = match revision {
                None => slot.state == PoolSlotState::Connected,
                Some(revision) => {
                    slot.state == PoolSlotState::Ready
                        && slot.registration.as_ref().is_some_and(|report| {
                            report.is_ready()
                                && report.registry_revision == revision
                                && Some(report.generation) == slot.generation
                        })
                }
            };
            if !selectable {
                continue;
            }
            if let Some(handle) = &state.handles[index] {
                if !handle.snapshot().closed {
                    let handle = handle.clone();
                    state.next = (index + 1) % state.handles.len();
                    return Ok(Some(handle));
                }
            }
        }
        Ok(None)
    }
    /// 开始一次真实连接轮询；终态不再发起连接。
    pub(crate) fn connecting(&self, slot: usize) -> bool {
        let mut state = self.state();
        if state.snapshot.closed {
            return false;
        }
        let stats = &mut state.snapshot.slots[slot];
        stats.state = PoolSlotState::Connecting;
        stats.attempts = stats.attempts.saturating_add(1);
        stats.reconnect_delay = None;
        drop(state);
        self.notify();
        true
    }
    /// 在配置槽位发布raw通道，关闭竞态时拒绝发布并由owner清理。
    pub(crate) fn connected(&self, slot: usize, handle: ConnectionHandle) -> bool {
        let mut state = self.state();
        if state.snapshot.closed {
            return false;
        }
        state.snapshot.slots[slot].state = PoolSlotState::Connected;
        state.snapshot.slots[slot].generation = Some(handle.generation());
        state.snapshot.slots[slot].reconnect_delay = None;
        state.handles[slot] = Some(handle);
        drop(state);
        self.notify();
        true
    }
    /// 参数为槽位和原代际句柄，跟踪初始化中的通道以便关闭，但绝不把它暴露给选择器。
    pub(crate) fn registering(&self, slot: usize, handle: ConnectionHandle) -> bool {
        let mut state = self.state();
        if state.snapshot.closed {
            return false;
        }
        let stats = &mut state.snapshot.slots[slot];
        stats.state = PoolSlotState::Registering;
        stats.generation = Some(handle.generation());
        stats.reconnect_delay = None;
        state.handles[slot] = Some(handle);
        drop(state);
        self.notify();
        true
    }
    /// 发布完整报告；只有当前修订/代际和开放通道全部吻合才返回true并进入Ready。
    pub(crate) fn registration_finished(
        &self,
        slot: usize,
        report: std::sync::Arc<SessionRegistrationReport>,
    ) -> bool {
        let mut state = self.state();
        if state.snapshot.closed {
            return false;
        }
        let current = self
            .registration
            .as_ref()
            .and_then(|registration| registration.revision().ok());
        let ready = report.is_ready()
            && current == Some(report.registry_revision)
            && state.handles[slot].as_ref().is_some_and(|handle| {
                handle.generation() == report.generation && !handle.snapshot().closed
            });
        let stats = &mut state.snapshot.slots[slot];
        stats.state = if ready {
            PoolSlotState::Ready
        } else if report.is_ready() {
            PoolSlotState::Registering
        } else {
            PoolSlotState::Degraded
        };
        stats.registration = Some(report);
        drop(state);
        self.notify();
        ready
    }
    /// 从可选表撤回旧代际；之后才允许监督器清理并等待重连。
    pub(crate) fn withdraw(
        &self,
        slot: usize,
        session: Option<ConnectionHandle>,
        reason: TransportError,
    ) {
        let mut state = self.state();
        // 极短连接可能在向可选表发布前终止，仍需纳入退出中计数。
        if let Some(handle) = state.handles[slot].take().or(session) {
            debug_assert!(state.retiring[slot].is_none());
            state.retiring[slot] = Some(handle);
        }
        if !state.snapshot.closed {
            let stats = &mut state.snapshot.slots[slot];
            stats.state = if self.registration.is_some()
                && stats
                    .registration
                    .as_ref()
                    .is_some_and(|report| !report.is_ready())
            {
                PoolSlotState::Degraded
            } else {
                PoolSlotState::Backoff
            };
            stats.last_failure = Some(reason);
        }
        drop(state);
        self.notify();
    }
    /// 原会话任务全部结束后，将退出中代际的最终计数恰好归档一次。
    pub(crate) fn retire(&self, slot: usize) {
        let mut state = self.state();
        if let Some(handle) = state.retiring[slot].take() {
            state
                .snapshot
                .cumulative
                .accumulate(PoolCounters::from_connection(&handle.snapshot()));
        }
        drop(state);
        self.notify();
    }
    /// 记录下次重连的正退避，不暴露业务数据或积累历史列表。
    pub(crate) fn backoff(&self, slot: usize, delay: Duration) {
        let mut state = self.state();
        if !state.snapshot.closed {
            state.snapshot.slots[slot].reconnect_delay = Some(delay);
        }
        drop(state);
        self.notify();
    }
    /// 永久停止所有槽位并同步关闭各当前代际；幂等保留首次池级原因。
    pub(crate) fn stop(&self, reason: TransportError) {
        let mut state = self.state();
        Self::stop_locked(&mut state, reason);
        drop(state);
        self.shutdown.send_replace(true);
        self.notify();
    }
    fn stop_locked(state: &mut PoolState, reason: TransportError) {
        if !state.snapshot.closed {
            state.snapshot.closed = true;
            state.snapshot.last_failure = Some(reason);
        }
        for handle in state.handles.iter().chain(&state.retiring).flatten() {
            handle.core.terminate(TransportError::PoolClosed);
        }
        for slot in &mut state.snapshot.slots {
            slot.state = PoolSlotState::Stopped;
            slot.reconnect_delay = None;
        }
    }
}
