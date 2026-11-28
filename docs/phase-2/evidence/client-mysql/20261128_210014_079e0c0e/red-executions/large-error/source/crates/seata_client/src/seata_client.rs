use crate::client_transport_policy::ClientTransportPolicy;
use crate::{ClientBuildError, ClientShutdownError};
use seata_rm::{FileBranchStore, ResourceCatalog, ResourceManager, RmError};
use seata_tm::{TmError, TransactionManager, TransactionManagerBuilder};
use seata_transport::{ConnectionPoolConfig, RegistrationIdentity};
use std::future::Future;
use std::time::{Duration, Instant};

/// 共同拥有 TM/RM 配置及生命周期的入口；资源仍由原 BranchResource 接入。
/// 对应 Java: GlobalTransactionScanner 初始化/销毁职责；Rust 统一入口扩展。
#[derive(Debug)]
pub struct SeataClient {
    tm: Option<TransactionManager>,
    rm: Option<ResourceManager>,
}

/// 统一入口的伴随构建器；必须显式选择角色与通道信任策略。
/// 对应 Java: GlobalTransactionScanner 与 NettyClientConfig 的组合职责。
#[derive(Debug)]
pub struct SeataClientBuilder {
    connection: ConnectionPoolConfig,
    identity: RegistrationIdentity,
    tm: bool,
    rm: Option<(ResourceCatalog, FileBranchStore)>,
    transport: Option<ClientTransportPolicy>,
    request_timeout: Duration,
    registration_timeout: Duration,
    callback_timeout: Duration,
    maximum_workers: usize,
    maximum_batch: usize,
    owner_stop_scope: Option<[u8; 32]>,
}

impl SeataClient {
    /// 参数为共用连接配置与登记身份；返回尚未选择角色或传输策略的构建器。
    pub fn builder(
        connection: ConnectionPoolConfig,
        identity: RegistrationIdentity,
    ) -> SeataClientBuilder {
        SeataClientBuilder {
            connection,
            identity,
            tm: false,
            rm: None,
            transport: None,
            request_timeout: Duration::from_secs(5),
            registration_timeout: Duration::from_secs(5),
            callback_timeout: Duration::from_secs(30),
            maximum_workers: 16,
            maximum_batch: 256,
            owner_stop_scope: None,
        }
    }
    /// 无参数；返回启用的 TM 借用，未选择 TM 时返回 None。
    pub fn transaction_manager(&self) -> Option<&TransactionManager> {
        self.tm.as_ref()
    }
    /// 无参数；返回启用的 RM 借用，未选择 RM 时返回 None。
    pub fn resource_manager(&self) -> Option<&ResourceManager> {
        self.rm.as_ref()
    }
    /// 消费入口并依次尝试关闭 TM/RM；返回两者的独立错误，不发送业务二阶段。
    pub async fn shutdown(self) -> Result<(), ClientShutdownError> {
        let Self { tm, rm } = self;
        // 两个拥有者同时进入 future；取消任一等待时，另一个仍会执行原 Drop。
        finish_shutdown(
            async move {
                match tm {
                    Some(tm) => tm.shutdown().await,
                    None => Ok(()),
                }
            },
            async move {
                match rm {
                    Some(rm) => rm.shutdown().await,
                    None => Ok(()),
                }
            },
        )
        .await
    }
}

impl SeataClientBuilder {
    /// 无参数；返回启用 TM 的构建器，重复调用幂等。
    pub fn enable_tm(mut self) -> Self {
        self.tm = true;
        self
    }
    /// 参数为原资源目录与专属日志；返回启用 RM 的构建器，不创建事务模式引擎。
    pub fn enable_rm(mut self, catalog: ResourceCatalog, store: FileBranchStore) -> Self {
        self.rm = Some((catalog, store));
        self
    }
    /// 无参数；显式允许两个角色使用明文，返回构建器。
    pub fn allow_plaintext(mut self) -> Self {
        self.transport = Some(ClientTransportPolicy::Plaintext);
        self
    }
    /// 参数为共用 TLS 身份与信任策略；返回构建器，每个角色均不回退明文。
    #[cfg(feature = "tls")]
    pub fn transport_tls(mut self, tls: seata_transport::TlsClientConfig) -> Self {
        self.transport = Some(ClientTransportPolicy::Tls(tls));
        self
    }
    /// 参数为两个角色共用的单次请求总预算；返回构建器。
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }
    /// 参数为两个角色共用的每代际登记预算；返回构建器。
    pub fn registration_timeout(mut self, timeout: Duration) -> Self {
        self.registration_timeout = timeout;
        self
    }
    /// 参数为 RM 回调与日志关闭等待预算；返回构建器。
    pub fn callback_timeout(mut self, timeout: Duration) -> Self {
        self.callback_timeout = timeout;
        self
    }
    /// 参数为 RM 并发回调数 1..4096；返回构建器。
    pub fn maximum_workers(mut self, maximum: usize) -> Self {
        self.maximum_workers = maximum;
        self
    }
    /// 参数为 RM 平坦批次条数 1..256；返回构建器。
    pub fn maximum_batch(mut self, maximum: usize) -> Self {
        self.maximum_batch = maximum;
        self
    }
    /// 参数为原 TM 持久来源停止证明信任域；返回构建器，不授予恢复控制权限。
    pub fn launcher_stop_scope(mut self, scope: [u8; 32]) -> Self {
        self.owner_stop_scope = Some(scope);
        self
    }
    /// 校验后先启动 RM 再启动 TM；失败时清理已启动角色并保留错误。
    /// 返回本地生命周期拥有者；TC 登记是否成功仍由实际连接状态表示。
    pub async fn build(self) -> Result<SeataClient, ClientBuildError> {
        self.start_with_tm(TransactionManagerBuilder::build).await
    }
    async fn start_with_tm(
        self,
        start: impl FnOnce(TransactionManagerBuilder) -> Result<TransactionManager, TmError>,
    ) -> Result<SeataClient, ClientBuildError> {
        if !self.tm && self.rm.is_none() {
            return Err(ClientBuildError::invalid(
                "at least one client role required",
            ));
        }
        let transport = self.transport.ok_or_else(|| {
            ClientBuildError::invalid("explicit client transport policy required")
        })?;
        for timeout in [
            self.request_timeout,
            self.registration_timeout,
            self.callback_timeout,
        ] {
            if timeout.is_zero() || Instant::now().checked_add(timeout).is_none() {
                return Err(ClientBuildError::invalid(
                    "positive finite client time budgets required",
                ));
            }
        }
        if !(1..=4096).contains(&self.maximum_workers) || !(1..=256).contains(&self.maximum_batch) {
            return Err(ClientBuildError::invalid(
                "bounded client callback budgets required",
            ));
        }
        let rm = if let Some((catalog, store)) = self.rm {
            let builder =
                ResourceManager::builder(self.connection, self.identity.clone(), catalog, store)
                    .request_timeout(self.request_timeout)
                    .registration_timeout(self.registration_timeout)
                    .callback_timeout(self.callback_timeout)
                    .maximum_workers(self.maximum_workers)
                    .maximum_batch(self.maximum_batch);
            #[cfg(feature = "tls")]
            let builder = match &transport {
                ClientTransportPolicy::Tls(tls) => builder.transport_tls(tls.clone()),
                ClientTransportPolicy::Plaintext => builder,
            };
            Some(builder.build().map_err(|rm| ClientBuildError {
                configuration: None,
                tm: None,
                rm: Some(rm),
            })?)
        } else {
            None
        };
        let tm = if self.tm {
            let builder = TransactionManager::builder(self.connection, self.identity)
                .request_timeout(self.request_timeout)
                .registration_timeout(self.registration_timeout);
            let builder = match self.owner_stop_scope {
                Some(scope) => builder.launcher_stop_scope(scope),
                None => builder,
            };
            #[cfg(feature = "tls")]
            let builder = match transport {
                ClientTransportPolicy::Tls(tls) => builder.transport_tls(tls),
                ClientTransportPolicy::Plaintext => builder,
            };
            #[cfg(not(feature = "tls"))]
            let _ = transport;
            match start(builder) {
                Ok(tm) => Some(tm),
                Err(tm) => {
                    // 清理错误不能覆盖初始化错误；清理等待取消仍触发原 RM Drop。
                    let cleanup = match rm {
                        Some(rm) => rm.shutdown().await.err(),
                        None => None,
                    };
                    return Err(ClientBuildError {
                        configuration: None,
                        tm: Some(tm),
                        rm: cleanup,
                    });
                }
            }
        } else {
            None
        };
        Ok(SeataClient { tm, rm })
    }
}

async fn finish_shutdown(
    tm: impl Future<Output = Result<(), TmError>>,
    rm: impl Future<Output = Result<(), RmError>>,
) -> Result<(), ClientShutdownError> {
    let tm = tm.await.err();
    let rm = rm.await.err();
    if tm.is_some() || rm.is_some() {
        Err(ClientShutdownError { tm, rm })
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "shutdown_tests.rs"]
mod shutdown_tests;
