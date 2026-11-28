use crate::{MySqlPrepareError, MySqlPrepareFailureReportError, MySqlPreparedTransaction};
use seata_protocol::transaction::BranchReportResponse;
use seata_rm::ResourceClient;

/// 准备失败及后续原 TC 报告的分层结果；始终保留原准备错误与登记回执。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 与 AbstractResourceManager#branchReport。
#[derive(Debug, thiserror::Error)]
#[error("MySQL preparation failed: {prepare}")]
pub struct MySqlPrepareReportedError {
    #[source]
    prepare: Box<MySqlPrepareError>,
    report: Option<Result<BranchReportResponse, MySqlPrepareFailureReportError>>,
}

impl MySqlPrepareReportedError {
    /// 参数为准备结果、调用方持有的空错误槽和原 RM 客户端；已确认回滚的错误只报告一次。
    /// 报告等待被取消时，错误槽仍保存原回执和已占用的门闩；调用方不得重试登记或报告。
    pub(crate) async fn complete(
        result: Result<MySqlPreparedTransaction, MySqlPrepareError>,
        failure_slot: &mut Option<MySqlPrepareError>,
        client: &ResourceClient,
    ) -> Result<MySqlPreparedTransaction, Self> {
        debug_assert!(failure_slot.is_none());
        match result {
            Ok(prepared) => Ok(prepared),
            Err(prepare) => {
                // 存入调用方作用域后才开始网络等待，取消 Future 不会丢弃原错误。
                *failure_slot = Some(prepare);
                // 先检查原连接回滚事实；未知本地结果绝不向 TC 宣称一阶段失败。
                let report = if let Some(prepare) = failure_slot
                    .as_ref()
                    .filter(|prepare| prepare.local_rollback_confirmed())
                {
                    Some(prepare.report_failure_after_rollback(client).await)
                } else {
                    None
                };
                match failure_slot.take() {
                    Some(prepare) => Err(Self {
                        prepare: Box::new(prepare),
                        report,
                    }),
                    None => unreachable!("error stored before report"),
                }
            }
        }
    }

    /// 无参数；返回原准备错误，其中保留已知的真实分支回执与回滚结果。
    pub fn prepare_error(&self) -> &MySqlPrepareError {
        self.prepare.as_ref()
    }

    /// 无参数；返回报告调用结果；None 表示本地回滚未确认或没有真实登记回执，未发送报告。
    pub fn report_result(
        &self,
    ) -> Option<&Result<BranchReportResponse, MySqlPrepareFailureReportError>> {
        self.report.as_ref()
    }
}
