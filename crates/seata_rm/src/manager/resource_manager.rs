use super::callback_handler::CallbackHandler;
use crate::{BranchDispatcher, FileBranchStore, ResourceCatalog, ResourceClient, RmError};
use seata_transport::{
    ConnectionPool, ConnectionPoolConfig, PoolSampler, PoolSnapshot, RegistrationIdentity,
    SessionRegistration,
};
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};

/// 拥有注册RM连接池、有限回调工作任务及关闭顺序的资源管理器；业务帧从不自动重放。
/// 对应 Java: RmNettyRemotingClient/DefaultResourceManager生命周期与AbstractRMHandler分派。
pub struct ResourceManager {
    client: ResourceClient,
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<Result<(), RmError>>>,
    store: FileBranchStore,
    shutdown_timeout: Duration,
}
/// ResourceManager伴随构建器；连接、登记、请求、回调与并发预算均显式校验。
/// 对应 Java: RM客户端配置及ResourceManager初始化职责。
#[derive(Debug)]
pub struct ResourceManagerBuilder {
    connection: ConnectionPoolConfig,
    identity: RegistrationIdentity,
    catalog: ResourceCatalog,
    store: FileBranchStore,
    request_timeout: Duration,
    registration_timeout: Duration,
    callback_timeout: Duration,
    maximum_workers: usize,
    maximum_batch: usize,
}
impl ResourceManager {
    /// 无参数；返回当前管理器及所有客户端克隆共享的只读出站调用计数。
    /// 它不持有连接池或发送能力；重建管理器会创建新的累计源。
    pub fn call_metrics_sampler(&self) -> crate::RmCallMetricsSampler {
        self.client.call_metrics_sampler()
    }

    /// 无参数；返回当前日志打开周期的只读分支采样器，不持有日志文件或分派能力。
    /// 同日志的手工和回调分派共享计数；状态 gauge 不等于 TC 活跃分支数量。
    pub fn metrics_sampler(&self) -> crate::RmMetricsSampler {
        self.store.metrics_sampler()
    }

    /// 参数为连接配置、RM身份、资源目录及专属日志；返回默认5秒请求/登记、30秒回调构建器。
    pub fn builder(
        connection: ConnectionPoolConfig,
        identity: RegistrationIdentity,
        catalog: ResourceCatalog,
        store: FileBranchStore,
    ) -> ResourceManagerBuilder {
        ResourceManagerBuilder {
            connection,
            identity,
            catalog,
            store,
            request_timeout: Duration::from_secs(5),
            registration_timeout: Duration::from_secs(5),
            callback_timeout: Duration::from_secs(30),
            maximum_workers: 16,
            maximum_batch: 256,
        }
    }
    /// 返回可克隆单次业务调用入口；管理器关闭后所有克隆同步失效，不获得重连所有权。
    pub fn client(&self) -> ResourceClient {
        self.client.clone()
    }
    /// 返回所拥有 RM 连接池的只读槽位、当前代际及池生命周期累计计数；不暴露业务通道。
    /// 返回值中的连接计数与池状态分别采样，不能当作跨锁原子瞬时值。
    pub fn connection_snapshot(&self) -> PoolSnapshot {
        self.client.pool.snapshot()
    }
    /// 返回可由观测服务持有的只读池采样能力；不授予分支请求或关闭权限。
    pub fn connection_sampler(&self) -> PoolSampler {
        self.client.pool.sampler()
    }
    /// 消费管理器，先关闭调用入口和socket，再取消/join回调及网络监督器，最后等待磁盘租约释放。
    /// 日志应专供此管理器；其他调用者保留租约会导致有界关闭返回错误，而不谎报全部清理。
    pub async fn shutdown(mut self) -> Result<(), RmError> {
        self.client.pool.close();
        self.stop.send_replace(true);
        let owner = if let Some(task) = self.task.as_mut() {
            match task.await {
                Ok(value) => value,
                Err(_) => Err(RmError::Unavailable {
                    reason: "RM owner task failed",
                }),
            }
        } else {
            Ok(())
        };
        self.task = None;
        tokio::time::timeout(self.shutdown_timeout, self.store.wait_idle())
            .await
            .map_err(|_| RmError::Unavailable {
                reason: "RM journal work has not drained",
            })??;
        owner
    }
}
impl Drop for ResourceManager {
    fn drop(&mut self) {
        self.client.pool.close();
        self.stop.send_replace(true);
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
impl std::fmt::Debug for ResourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceManager")
            .field("client", &self.client)
            .field("owned", &self.task.is_some())
            .finish()
    }
}
impl ResourceManagerBuilder {
    /// 参数为单次出站及响应写入预算，返回更新后的构建器。
    pub fn request_timeout(mut self, value: Duration) -> Self {
        self.request_timeout = value;
        self
    }
    /// 参数为每轮原生身份/资源登记总预算，返回更新后的构建器。
    pub fn registration_timeout(mut self, value: Duration) -> Self {
        self.registration_timeout = value;
        self
    }
    /// 参数为一整个回调批次预算及关闭等待预算，返回更新后的构建器。
    pub fn callback_timeout(mut self, value: Duration) -> Self {
        self.callback_timeout = value;
        self
    }
    /// 参数为有限并发回调数(1..4096)，返回更新后的构建器。
    pub fn maximum_workers(mut self, value: usize) -> Self {
        self.maximum_workers = value;
        self
    }
    /// 参数为平坦回调批次条数(1..256)，返回更新后的构建器。
    pub fn maximum_batch(mut self, value: usize) -> Self {
        self.maximum_batch = value;
        self
    }
    /// 校验全部预算后启动拥有网络/回调任务的RM；资源目录已安装的处理器先于登记修订可见。
    pub fn build(self) -> Result<ResourceManager, RmError> {
        for value in [
            self.request_timeout,
            self.registration_timeout,
            self.callback_timeout,
        ] {
            if value.is_zero() || std::time::Instant::now().checked_add(value).is_none() {
                return Err(RmError::Invalid {
                    reason: "positive finite RM time budget required",
                });
            }
        }
        if !(1..=4096).contains(&self.maximum_workers)
            || !(1..=256).contains(&self.maximum_batch)
            || !self.store.can_drain()
        {
            return Err(RmError::Invalid {
                reason: "invalid callback or journal drain budget",
            });
        }
        let registration = SessionRegistration::rm(
            self.identity,
            self.catalog.registry(),
            self.registration_timeout,
        )
        .map_err(|_| RmError::Invalid {
            reason: "invalid RM registration policy",
        })?;
        let pool =
            ConnectionPool::start_registered(self.connection, registration).map_err(|_| {
                RmError::Unavailable {
                    reason: "RM registered pool unavailable",
                }
            })?;
        let client = ResourceClient {
            metrics: super::RmCallMetricsSampler::new(),
            pool: pool.handle(),
            catalog: self.catalog.clone(),
            timeout: self.request_timeout,
        };
        let handler = CallbackHandler {
            dispatcher: BranchDispatcher::new(self.catalog, self.store.clone()),
            maximum_batch: self.maximum_batch,
            callback_timeout: self.callback_timeout,
            response_timeout: self.request_timeout,
        };
        let (stop, receiver) = watch::channel(false);
        let task = tokio::spawn(serve(pool, handler, receiver, self.maximum_workers));
        Ok(ResourceManager {
            client,
            stop,
            task: Some(task),
            store: self.store,
            shutdown_timeout: self.callback_timeout,
        })
    }
}
async fn serve(
    mut pool: ConnectionPool,
    handler: CallbackHandler,
    mut stop: watch::Receiver<bool>,
    maximum: usize,
) -> Result<(), RmError> {
    let mut workers = JoinSet::new();
    let mut failed = false;
    loop {
        if *stop.borrow() {
            break;
        }
        tokio::select! {biased;
            _=stop.changed()=>break,
            result=workers.join_next(),if !workers.is_empty()=>{if matches!(result,Some(Err(_))) {failed=true;break;}},
            event=pool.next_inbound(),if workers.len()<maximum=>match event {Some(event)=>{let handler=handler.clone();workers.spawn(async move {handler.handle(event).await;});},None=>break},
        }
    }
    pool.handle().close();
    workers.abort_all();
    while let Some(result) = workers.join_next().await {
        if result.is_err_and(|error| !error.is_cancelled()) {
            failed = true;
        }
    }
    pool.shutdown().await.map_err(|_| RmError::Unavailable {
        reason: "RM network tasks failed during shutdown",
    })?;
    if failed {
        Err(RmError::Unavailable {
            reason: "RM callback worker panicked",
        })
    } else {
        Ok(())
    }
}
