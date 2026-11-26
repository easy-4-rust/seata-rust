use crate::{
    Propagation, RootContext, TmError, TransactionCompletion, TransactionConfig, TransactionError,
    TransactionManager, TransactionResult,
};
use std::future::Future;
use tokio::time::Instant;
/// 正式TM的业务作用域执行器，负责传播/本地期限与一次全局意图，不负责TC重试调度。
/// 对应 Java: org.apache.seata.tm.api.TransactionalTemplate
#[derive(Debug)]
pub struct TransactionalTemplate<'a> {
    manager: &'a TransactionManager,
}
impl<'a> TransactionalTemplate<'a> {
    /// 参数为拥有登记连接池的TM管理器；返回无独立后台任务的执行器。
    pub const fn new(manager: &'a TransactionManager) -> Self {
        Self { manager }
    }
    /// 参数为事务配置及业务future工厂；在最终角色上下文内才构造业务，返回值与TC观察分开。
    pub async fn execute<T, E, F: Future<Output = Result<T, E>>>(
        &self,
        config: TransactionConfig,
        business: impl FnOnce() -> F,
    ) -> Result<TransactionResult<T>, TransactionError<E>> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(TmError::InvalidConfiguration {
                reason: "Tokio I/O and time runtime required",
            }
            .into());
        }
        let outer = RootContext::current();
        let (launch, context) = match (config.propagation(), outer) {
            (Propagation::Required, None) | (Propagation::RequiresNew, _) => (true, None),
            (
                Propagation::Required | Propagation::Mandatory | Propagation::Supports,
                Some(context),
            ) => (false, Some(context.into_participant())),
            (Propagation::Supports | Propagation::NotSupported | Propagation::Never, None)
            | (Propagation::NotSupported, Some(_)) => (false, None),
            (Propagation::Mandatory, None) => {
                return Err(TmError::IllegalState {
                    xid: None,
                    reason: "Mandatory requires an existing transaction",
                }
                .into())
            }
            (Propagation::Never, Some(context)) => {
                return Err(TmError::IllegalState {
                    xid: Some(context.xid().clone()),
                    reason: "Never forbids an existing transaction",
                }
                .into())
            }
        };
        if !launch {
            let xid = context.as_ref().map(|context| context.xid().clone());
            let deadline = context.as_ref().and_then(|context| context.deadline());
            let completion = xid.clone().map_or(
                TransactionCompletion::Unmanaged,
                TransactionCompletion::Participant,
            );
            return RootContext::scope_owned(context, async {
                match run_business(deadline, business).await {
                    Some(Ok(value)) => Ok(TransactionResult { value, completion }),
                    Some(Err(error)) => Err(TransactionError::Business {
                        error,
                        xid,
                        rollback: None,
                    }),
                    None => match xid {
                        Some(xid) => Err(TransactionError::Deadline {
                            xid,
                            rollback: Ok(completion),
                        }),
                        None => Err(TmError::InvalidConfiguration {
                            reason: "unmanaged scope cannot have a transaction deadline",
                        }
                        .into()),
                    },
                }
            })
            .await;
        }
        RootContext::scope_owned(None, async {
            let mut transaction = self.manager.begin(&config).await?;
            let context = transaction.context();
            let deadline = context.deadline();
            RootContext::scope_owned(Some(context), async {
                match run_business(deadline, business).await {
                    Some(Ok(value)) => match transaction.commit().await {
                        Ok(completion) => Ok(TransactionResult { value, completion }),
                        Err(TmError::TransactionDeadlineExceeded { .. }) => {
                            Err(TransactionError::Deadline {
                                xid: transaction.xid().clone(),
                                rollback: transaction.rollback().await.map_err(Box::new),
                            })
                        }
                        Err(error) => Err(error.into()),
                    },
                    Some(Err(error)) => Err(TransactionError::Business {
                        error,
                        xid: Some(transaction.xid().clone()),
                        rollback: Some(transaction.rollback().await.map_err(Box::new)),
                    }),
                    None => Err(TransactionError::Deadline {
                        xid: transaction.xid().clone(),
                        rollback: transaction.rollback().await.map_err(Box::new),
                    }),
                }
            })
            .await
        })
        .await
    }
}

async fn run_business<T, E, F: Future<Output = Result<T, E>>>(
    deadline: Option<Instant>,
    business: impl FnOnce() -> F,
) -> Option<Result<T, E>> {
    match deadline {
        None => Some(business().await),
        Some(deadline) => {
            // Tokio的timeout可能先poll已就绪的内部future；显式检查阻止过期业务构造/成功。
            if deadline <= Instant::now() {
                return None;
            }
            match tokio::time::timeout_at(deadline, async { business().await }).await {
                Ok(Err(error)) => Some(Err(error)),
                Ok(Ok(value)) if Instant::now() < deadline => Some(Ok(value)),
                _ => None,
            }
        }
    }
}
