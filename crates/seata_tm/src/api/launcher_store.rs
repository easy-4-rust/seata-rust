use super::LauncherOrigin;
use crate::TmOperation;

/// TM 来源与结束发送门禁；实现者须仅在持久提交确认后返回 Ok。
/// 对应 Java: DefaultGlobalTransaction 的 Begin/Commit/Rollback 调用边界。
#[allow(async_fn_in_trait)]
pub trait LauncherStore {
    /// 保存 Begin 来源、TC 传输信任摘要及期限；失败或提交结果不明时返回错误。
    async fn record_begin(
        &self,
        origin: &LauncherOrigin,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// 以单所有者条件记录一次 Commit 或 Rollback 意图；失败时禁止发送。
    async fn record_end_intent(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// 以单所有者条件置位不可清除的发送标记；失败时禁止发送。
    async fn mark_send_started(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
