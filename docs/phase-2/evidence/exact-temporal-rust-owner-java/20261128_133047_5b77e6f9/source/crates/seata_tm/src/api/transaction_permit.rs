use std::sync::Arc;
use tokio::sync::{Notify, OwnedSemaphorePermit};

/// 一个已接纳应用作用域的独占许可；应持有到业务完成或持久交接之后。
/// Drop 只释放本地准入容量，不对未知事务结果作任何推断。
/// 对应 Java: 无直接对象；WP07 应用作用域生命周期扩展。
#[derive(Debug)]
#[must_use = "许可必须由整个业务作用域持有"]
pub struct TransactionPermit {
    permit: Option<OwnedSemaphorePermit>,
    released: Arc<Notify>,
}

impl TransactionPermit {
    pub(super) fn new(permit: OwnedSemaphorePermit, released: Arc<Notify>) -> Self {
        Self {
            permit: Some(permit),
            released,
        }
    }
}

impl Drop for TransactionPermit {
    fn drop(&mut self) {
        // 先归还容量，再唤醒所有排空等待者，确保观察到最新在途数量。
        drop(self.permit.take());
        self.released.notify_waiters();
    }
}
