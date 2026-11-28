use super::pool_core::PoolCore;
use crate::{CallError, ConnectionHandle, PoolSampler, PoolSnapshot, TransportError};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::Message;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
/// 可克隆的raw池选择入口，不拥有新socket或事务重放权；旧通道句柄不会迁移代际。
/// 对应 Java: NettyClientChannelManager#acquireChannel及remoting单次发送选择职责。
#[derive(Clone)]
pub struct PoolHandle {
    pub(crate) core: Arc<PoolCore>,
}
impl PoolHandle {
    /// 同步关闭池选择入口及全部当前连接，唤醒等待；监督任务仍由ConnectionPool所有者join。
    /// 对应 Java: NettyClientChannelManager关闭入口；不创建重连或重放业务请求。
    pub fn close(&self) {
        self.core.stop(TransportError::PoolClosed);
    }
    /// 返回固定槽位、当前代际计数及池生命周期累计计数；两者不可再次相加。
    pub fn snapshot(&self) -> PoolSnapshot {
        self.core.snapshot()
    }
    /// 返回仅允许采样的独立能力，不授予获取连接、发送请求或关闭池的权限。
    pub fn sampler(&self) -> PoolSampler {
        PoolSampler {
            core: Arc::clone(&self.core),
        }
    }
    /// 参数为绝对预算的相对时长；返回开放raw通道，不保证随后不会断开或已完成业务初始化。
    pub async fn acquire(&self, timeout: Duration) -> Result<ConnectionHandle, CallError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .filter(|_| !timeout.is_zero())
            .ok_or_else(|| CallError::local(None, None, TransportError::PoolAcquireTimeout))?;
        self.acquire_until(deadline).await
    }
    async fn acquire_until(&self, deadline: Instant) -> Result<ConnectionHandle, CallError> {
        let mut changed = self.core.subscribe();
        loop {
            if self.snapshot().closed {
                return Err(CallError::local(None, None, TransportError::PoolClosed));
            }
            if deadline <= Instant::now() {
                return Err(CallError::local(
                    None,
                    None,
                    TransportError::PoolAcquireTimeout,
                ));
            }
            match self.core.choose() {
                Ok(Some(handle)) => {
                    if deadline <= Instant::now() {
                        return Err(CallError::local(
                            None,
                            None,
                            TransportError::PoolAcquireTimeout,
                        ));
                    }
                    return Ok(handle);
                }
                Ok(None) => {}
                Err(reason) => return Err(CallError::local(None, None, reason)),
            }
            tokio::select! {biased; _=tokio::time::sleep_until(deadline)=>return Err(CallError::local(None,None,TransportError::PoolAcquireTimeout)),result=changed.changed()=>if result.is_err() {return Err(CallError::local(None,None,TransportError::PoolClosed));}}
        }
    }
    /// 参数为业务请求及包含获取时间的传输预算；只在一个选中代际调用一次request，任何错误均不重发。
    pub async fn request(&self, body: Message, timeout: Duration) -> Result<RpcMessage, CallError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .filter(|_| !timeout.is_zero())
            .ok_or_else(|| CallError::local(None, None, TransportError::PoolAcquireTimeout))?;
        let handle = self.acquire_until(deadline).await?;
        handle.request_until(body, deadline).await
    }
}

#[cfg(test)]
#[path = "pool_handle_tests.rs"]
mod tests;
impl std::fmt::Debug for PoolHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoolHandle")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}
