use super::pool_core::PoolCore;
use super::pool_worker_guard::PoolWorkerGuard;
use super::reconnect_backoff::ReconnectBackoff;
use crate::{
    CallError, ConnectionPoolConfig, PoolHandle, PoolInbound, SessionRegistration, TcpConnection,
    TransportError,
};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;
/// 固定端点/槽位的raw池所有者；明确拥有每个监督任务与入站队列，Connected不等于RMready。
/// 对应 Java: org.apache.seata.core.rpc.netty.NettyClientChannelManager的池生命周期职责。
pub struct ConnectionPool {
    handle: PoolHandle,
    inbound: mpsc::Receiver<PoolInbound>,
    tasks: Vec<JoinHandle<()>>,
}
impl ConnectionPool {
    /// 参数为已验证配置；返回启动固定槽位监督器的所有者，不等待认证、不发送业务请求。
    pub fn start(config: ConnectionPoolConfig) -> Result<Self, TransportError> {
        Self::start_inner(config, None)
    }
    /// 参数为配置和单角色原生登记策略；每个新会话/资源修订完成全部所需登记后才可获取。
    pub fn start_registered(
        config: ConnectionPoolConfig,
        registration: SessionRegistration,
    ) -> Result<Self, TransportError> {
        Self::start_inner(config, Some(registration))
    }
    fn start_inner(
        config: ConnectionPoolConfig,
        registration: Option<SessionRegistration>,
    ) -> Result<Self, TransportError> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(TransportError::InvalidConfiguration {
                reason: "Tokio I/O and time runtime required",
            });
        }
        let core = Arc::new(match registration {
            None => PoolCore::new(config),
            Some(registration) => PoolCore::with_registration(config, registration),
        });
        let (sender, inbound) = mpsc::channel(config.inbound);
        let mut owner = Self {
            handle: PoolHandle {
                core: Arc::clone(&core),
            },
            inbound,
            tasks: Vec::with_capacity(config.slots),
        };
        for slot in 0..config.slots {
            let guard = PoolWorkerGuard::new(Arc::clone(&core));
            owner.tasks.push(tokio::spawn(supervise_slot(
                Arc::clone(&core),
                slot,
                sender.clone(),
                core.stop_receiver(),
                guard,
            )));
        }
        Ok(owner)
    }
    /// 返回可克隆的选择入口，不转移连接和监督任务的所有权。
    pub fn handle(&self) -> PoolHandle {
        self.handle.clone()
    }
    /// 返回下一条带原代际的入站请求；池关闭后仍可读取先前入队的合法事件。
    pub async fn next_inbound(&mut self) -> Option<PoolInbound> {
        self.inbound.recv().await
    }
    /// 消费池，停止连接尝试、关闭所有代际并等待监督器；取消本future仍触发所有者Drop。
    pub async fn shutdown(mut self) -> Result<(), TransportError> {
        self.handle.core.stop(TransportError::PoolClosed);
        let mut failed = false;
        for task in &mut self.tasks {
            if task.await.is_err() {
                failed = true;
            }
        }
        self.tasks.clear();
        if failed {
            Err(TransportError::InternalState {
                reason: "pool supervisor task failed",
            })
        } else {
            Ok(())
        }
    }
}
impl Drop for ConnectionPool {
    fn drop(&mut self) {
        self.handle.core.stop(TransportError::PoolClosed);
        for task in &self.tasks {
            task.abort();
        }
    }
}
impl std::fmt::Debug for ConnectionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionPool")
            .field("handle", &self.handle)
            .field("owned_supervisors", &self.tasks.len())
            .finish()
    }
}

async fn supervise_slot(
    core: Arc<PoolCore>,
    slot: usize,
    incoming: mpsc::Sender<PoolInbound>,
    mut stop: watch::Receiver<bool>,
    _guard: PoolWorkerGuard,
) {
    let mut backoff = ReconnectBackoff::new(core.config.reconnect);
    loop {
        if *stop.borrow() {
            return;
        }
        let attempt = async {
            if !core.connecting(slot) {
                return None;
            }
            Some(TcpConnection::connect(core.config.endpoint, core.config.connection).await)
        };
        let result = tokio::select! {biased; _=stop.changed()=>return,result=attempt=>match result {Some(value)=>value,None=>return}};
        let reason = match result {
            Err(
                CallError::LocalFailureBeforeSend { reason, .. }
                | CallError::UnknownOutcome { reason, .. },
            ) => reason,
            Ok(mut connection) => {
                let (reason, healthy_duration) =
                    run_session(&core, slot, &mut connection, &incoming, &mut stop).await;
                core.withdraw(slot, Some(connection.handle()), reason.clone());
                backoff.connection_ended(healthy_duration);
                // 旧会话完成关闭/任务join后才可新建，禁止每槽位同时存活两个代际。
                let _ = connection.shutdown().await;
                core.retire(slot);
                reason
            }
        };
        if *stop.borrow() {
            return;
        }
        core.withdraw(slot, None, reason);
        let delay = backoff.next_delay();
        core.backoff(slot, delay);
        tokio::select! {biased; _=stop.changed()=>return,_=tokio::time::sleep(delay)=>{}}
    }
}

async fn registry_changed(
    receiver: &mut Option<watch::Receiver<u64>>,
) -> Result<(), TransportError> {
    match receiver {
        None => std::future::pending().await,
        Some(receiver) => receiver
            .changed()
            .await
            .map_err(|_| TransportError::InternalState {
                reason: "resource registry change channel closed",
            }),
    }
}

async fn run_session(
    core: &PoolCore,
    slot: usize,
    connection: &mut TcpConnection,
    incoming: &mpsc::Sender<PoolInbound>,
    stop: &mut watch::Receiver<bool>,
) -> (TransportError, std::time::Duration) {
    let handle = connection.handle();
    let mut changes = core
        .registration
        .as_ref()
        .map(SessionRegistration::subscribe);
    let mut healthy = std::time::Duration::ZERO;
    loop {
        if *stop.borrow() {
            return (TransportError::PoolClosed, healthy);
        }
        if let Some(registration) = &core.registration {
            if !core.registering(slot, handle.clone()) {
                return (TransportError::PoolClosed, healthy);
            }
            let report = tokio::select! {biased;
                _=stop.changed()=>return (TransportError::PoolClosed,healthy),
                change=registry_changed(&mut changes)=>match change {Ok(())=>continue,Err(reason)=>return(reason,healthy)},
                result=registration.initialize(&handle)=>match result {Ok(report)=>std::sync::Arc::new(report),Err(reason)=>return(reason,healthy)}
            };
            let failure = report.failure_reason();
            let ready = core.registration_finished(slot, report);
            if let Some(reason) = failure {
                return (reason, healthy);
            }
            if !ready {
                if let Some(reason) = handle.snapshot().last_failure {
                    return (reason, healthy);
                }
                continue;
            }
        } else if !core.connected(slot, handle.clone()) {
            return (TransportError::PoolClosed, healthy);
        }
        let since = Instant::now();
        let reason = loop {
            tokio::select! {biased;
                _=stop.changed()=>break Some(TransportError::PoolClosed),
                change=registry_changed(&mut changes)=>match change {Ok(())=>break None,Err(reason)=>break Some(reason)},
                reason=handle.closed()=>break Some(reason),
                event=connection.next_inbound()=>match event {
                    None=>break Some(TransportError::ConnectionClosed),
                    Some(message)=>if incoming.try_send(PoolInbound {connection:handle.clone(),message}).is_err() {handle.core.terminate(TransportError::Overloaded);break Some(TransportError::Overloaded);}
                }
            }
        };
        healthy = healthy.max(since.elapsed());
        if let Some(reason) = reason {
            return (reason, healthy);
        }
    }
}
