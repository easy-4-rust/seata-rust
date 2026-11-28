use super::TransactionPermit;
use crate::AdmissionError;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Notify, Semaphore, TryAcquireError};

/// 应用事务作用域的有界准入与停机排空入口，所有克隆共享同一关闭边界。
/// 此入口不注册信号处理器，不取消业务，也不自动提交、回滚或重试事务。
/// 对应 Java: 无直接对象；WP07 应用停机准入扩展。
///
/// 应用应在进入 TM 业务前取得许可，持有到结果及未知身份已经处理之后：
///
///     use seata_tm::{AdmissionError, TransactionAdmission, TransactionManager};
///     use std::{error::Error, time::Duration};
///
///     async fn serve_scope<T, F: std::future::Future<Output = T>>(
///         gate: &TransactionAdmission,
///         work: impl FnOnce() -> F,
///     ) -> Result<T, AdmissionError> {
///         let _permit = gate.try_enter()?;
///         Ok(work().await)
///     }
///
///     async fn stop(
///         gate: &TransactionAdmission,
///         owner: &mut Option<TransactionManager>,
///     ) -> Result<(), Box<dyn Error>> {
///         gate.drain(Duration::from_secs(30)).await?;
///         if let Some(manager) = owner.take() {
///             manager.shutdown().await?;
///         }
///         Ok(())
///     }
///
/// 业务 Future 被取消时，许可仍会释放；应用必须另行保存原事务的恢复信息。
#[derive(Debug, Clone)]
pub struct TransactionAdmission {
    semaphore: Arc<Semaphore>,
    released: Arc<Notify>,
    capacity: usize,
}

impl TransactionAdmission {
    /// 使用正数容量创建入口；超出信号量范围时返回 InvalidCapacity。
    /// 参数 capacity 是允许同时持有许可的作用域上限；返回共享准入入口。
    pub fn new(capacity: usize) -> Result<Self, AdmissionError> {
        if capacity == 0 || capacity > Semaphore::MAX_PERMITS {
            return Err(AdmissionError::InvalidCapacity);
        }
        Ok(Self {
            semaphore: Arc::new(Semaphore::new(capacity)),
            released: Arc::new(Notify::new()),
            capacity,
        })
    }

    /// 尝试接纳一个作用域，返回应持有到业务完成或持久交接之后的许可。
    /// 无参数；容量已满或入口关闭时返回不同错误，不等待且不发送事务请求。
    pub fn try_enter(&self) -> Result<TransactionPermit, AdmissionError> {
        let permit = self
            .semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|error| match error {
                TryAcquireError::Closed => AdmissionError::Closed,
                TryAcquireError::NoPermits => AdmissionError::AtCapacity,
            })?;
        Ok(TransactionPermit::new(permit, self.released.clone()))
    }

    /// 关闭所有克隆的准入入口；无参数、无返回值，已接纳作用域仍持有许可。
    /// 多次调用安全，不会隐式关闭 TM/RM 或修改 TC 事务状态。
    pub fn close(&self) {
        self.semaphore.close();
    }

    /// 返回观察时仍由应用持有的许可数量；无参数，并发释放可立即改变此值。
    pub fn active(&self) -> usize {
        self.capacity - self.semaphore.available_permits()
    }

    /// 关闭准入并最多等待 timeout，全部许可释放时返回成功。
    /// 超时返回当时在途数量；取消等待仍保持关闭且不撤销许可。
    /// 许可释放只证明本地作用域退出，未知事务结果仍需独立持久化和恢复。
    pub async fn drain(&self, timeout: Duration) -> Result<(), AdmissionError> {
        self.close();
        let wait = async {
            loop {
                // 先登记通知再读数量，避免最后一个许可在检查与等待之间释放。
                let notified = self.released.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                if self.active() == 0 {
                    return;
                }
                notified.await;
            }
        };
        tokio::time::timeout(timeout, wait)
            .await
            .map_err(|_| AdmissionError::DrainTimeout {
                active: self.active(),
            })
    }
}
