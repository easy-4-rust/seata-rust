use super::{DurableTmError, LauncherOrigin, LauncherStore};
use crate::{GlobalTransaction, TmOperation, TransactionCompletion, Xid};
use std::future::Future;

/// Begin 来源已确认的 Launcher；结束发送前持久记录操作与发送标记。
/// 对应 Java: DefaultGlobalTransaction 的 Launcher 生命周期。
pub struct DurableGlobalTransaction<'a, S: LauncherStore> {
    pub(crate) inner: GlobalTransaction<'a>,
    pub(crate) store: &'a S,
    pub(crate) origin: LauncherOrigin,
}
impl<S: LauncherStore> DurableGlobalTransaction<'_, S> {
    /// 返回 TC 原始 XID。
    pub fn xid(&self) -> &Xid {
        self.inner.xid()
    }
    /// 在原 XID 作用域执行业务工厂，退出后恢复外层上下文。
    pub async fn scope<F: Future>(&self, business: impl FnOnce() -> F) -> F::Output {
        self.inner.scope(business).await
    }
    /// 保存 Commit 意图及发送标记后，仅发送一次 Commit。
    pub async fn commit(mut self) -> Result<TransactionCompletion, DurableTmError> {
        self.end(TmOperation::Commit).await
    }
    /// 保存 Rollback 意图及发送标记后，仅发送一次 Rollback。
    pub async fn rollback(mut self) -> Result<TransactionCompletion, DurableTmError> {
        self.end(TmOperation::Rollback).await
    }
    async fn end(
        &mut self,
        operation: TmOperation,
    ) -> Result<TransactionCompletion, DurableTmError> {
        let xid = self.origin.xid.clone();
        self.store
            .record_end_intent(&self.origin, operation)
            .await
            .map_err(|source| DurableTmError::Persistence {
                stage: "end intent",
                xid: xid.clone(),
                source,
            })?;
        self.store
            .mark_send_started(&self.origin, operation)
            .await
            .map_err(|source| DurableTmError::Persistence {
                stage: "send marker",
                xid,
                source,
            })?;
        match operation {
            TmOperation::Commit => self.inner.commit().await.map_err(|mut error| {
                error.revoke_recovery();
                error.into()
            }),
            TmOperation::Rollback => self.inner.rollback().await.map_err(|mut error| {
                error.revoke_recovery();
                error.into()
            }),
            _ => unreachable!("durable end only accepts commit or rollback"),
        }
    }
}
