use crate::{
    mysql_tcc_context_store::MySqlTccContextStore, DbError, MySqlDataSource,
    MySqlTccContextReportError,
};
use seata_protocol::{transaction::BranchReportRequest, BranchStatus, BranchType};
use seata_rm::{RegisteredBranch, ResourceClient};
use seata_tcc::BusinessActionContext;
use std::{
    fmt,
    sync::{Arc, Mutex},
};
/// 当前 Try 共享的报告可能发送边界和原类型化错误；不作为持久完成状态。
pub(crate) type ContextReportState = Arc<Mutex<(bool, Option<MySqlTccContextReportError>)>>;
/// 绑定原成功回执和一次 Try 事务的发布者；每版原字节先持久化，再等待一次原报告回执。
/// 对应 Java: BusinessActionContextUtil#reportContext；Rust 显式异步发布与持久版本扩展。
pub(crate) struct MySqlTccContextReporter {
    source: MySqlDataSource,
    client: ResourceClient,
    receipt: RegisteredBranch,
    revision: u16,
    last_data: String,
    pub(crate) state: ContextReportState,
}
impl fmt::Debug for MySqlTccContextReporter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlTccContextReporter")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}
impl MySqlTccContextReporter {
    /// 参数为原数据源、正式客户端、原回执、初始原值快照及共享发送边界；返回当前 Try 的发布能力。
    pub(crate) fn new(
        source: MySqlDataSource,
        client: ResourceClient,
        receipt: RegisteredBranch,
        last_data: String,
        state: ContextReportState,
    ) -> Self {
        Self {
            source,
            client,
            receipt,
            revision: 0,
            last_data,
            state,
        }
    }
    /// 参数为可变业务上下文；确认原响应后才清除本地标志，错误不能解释为 TC 未收到报告。
    pub(crate) async fn publish(
        &mut self,
        context: &mut BusinessActionContext,
    ) -> Result<bool, MySqlTccContextReportError> {
        let id = self.receipt.identity();
        if context.xid() != Some(id.xid())
            || context.branch_id() != id.branch_id()
            || context.action_name() != Some(id.resource().resource_id())
            || !context.use_common_fence()
        {
            return Err(DbError::RecoveryConflict.into());
        }
        let app = context.to_application_data().map_err(|_| DbError::Input {
            reason: "TCC report context rejected",
        })?;
        if context.get_updated() != Some(true) && app == self.last_data {
            return Ok(false);
        }
        self.revision =
            MySqlTccContextStore::append(&self.source, &self.receipt, self.revision, &app).await?;
        self.state
            .lock()
            .map_err(|_| DbError::Input {
                reason: "TCC report state unavailable",
            })?
            .0 = true;
        self.client
            .branch_report(BranchReportRequest {
                xid: Some(id.xid().into()),
                branch_id: id.branch_id(),
                status: BranchStatus::Registered,
                resource_id: None,
                application_data: Some(app.clone()),
                branch_type: BranchType::Tcc,
            })
            .await?;
        self.last_data = app;
        context.set_updated(None);
        Ok(true)
    }
}
