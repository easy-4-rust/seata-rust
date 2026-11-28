use crate::{DbError, MySqlDataSource, MySqlXaTransaction};
use seata_rm::BranchIdentity;
use sqlx::Connection;
use std::{future::Future, sync::Arc, time::Duration};
use tokio::sync::{watch, Mutex};

/// 本库实际持有的独占 XA 会话槽与持久决定监视器；取消只关闭该槽，不使用远程连接编号。
/// 对应 Java: 无直接对象；Rust XA 旧作用域取消和资源围栏的生命周期扩展。
#[derive(Debug)]
pub(crate) struct MySqlXaSession {
    /// 业务调用在任何 await 前移走连接，取消/失败不得恢复槽。
    pub(crate) transaction: Mutex<Option<MySqlXaTransaction>>,
    closed: watch::Sender<bool>,
}
impl MySqlXaSession {
    /// 参数为原数据源、不可变身份和已取得 guard 的事务；返回独占槽并启动只持弱引用的有界观察。
    pub(crate) fn new(
        source: MySqlDataSource,
        identity: BranchIdentity,
        transaction: MySqlXaTransaction,
    ) -> Arc<Self> {
        let (closed, _) = watch::channel(false);
        let session = Arc::new(Self {
            transaction: Mutex::new(Some(transaction)),
            closed,
        });
        let weak = Arc::downgrade(&session);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                if weak.strong_count() == 0 {
                    return;
                }
                let observed = tokio::time::timeout(
                    source.operation_timeout(),
                    Self::must_close(&source, &identity),
                )
                .await;
                let Some(session) = weak.upgrade() else {
                    return;
                };
                if !matches!(observed, Ok(Ok(false))) {
                    session.closed.send_replace(true);
                    // 在途调用收到取消后丢弃原连接；空闲时由监视器直接退休本库持有的连接。
                    session.transaction.lock().await.take();
                    return;
                }
                // 不让监视器延长已被调用方丢弃或消费的作用域寿命。
                drop(session);
            }
        });
        session
    }

    async fn must_close(source: &MySqlDataSource, id: &BranchIdentity) -> Result<bool, DbError> {
        let options = source.control_pool().connect_options();
        let mut connection = sqlx::MySqlConnection::connect_with(&options)
            .await
            .map_err(|e| DbError::driver("XA session monitor connect", e))?;
        let row: Option<(u8, u8)> = sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_branch_v1 WHERE xid=? AND branch_id=? AND resource_id=? AND server_uuid=? AND schema_version=1 AND (application_data <=> ?) AND @@GLOBAL.server_uuid=?")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes()).bind(source.server_uuid.as_bytes())
            .bind(id.application_data().map(str::as_bytes)).bind(source.server_uuid.as_ref())
            .fetch_optional(&mut connection).await.map_err(|e| DbError::driver("XA session monitor decision", e))?;
        connection
            .close()
            .await
            .map_err(|e| DbError::driver("XA session monitor close", e))?;
        Ok(row != Some((0, 0)))
    }

    /// 参数为原独占作用域操作；在决定绑定/观察失败时取消该操作，返回关闭错误，绝不授权继续业务。
    pub(crate) async fn while_open<F: Future>(&self, operation: F) -> Result<F::Output, DbError> {
        let mut closed = self.closed.subscribe();
        if *closed.borrow_and_update() {
            return Err(DbError::RecoveryConflict);
        }
        tokio::select! {
            biased;
            _ = closed.changed() => Err(DbError::RecoveryConflict),
            result = operation => Ok(result),
        }
    }
}
