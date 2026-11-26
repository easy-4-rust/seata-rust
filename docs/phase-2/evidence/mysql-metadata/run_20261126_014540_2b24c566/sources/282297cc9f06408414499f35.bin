use crate::{CallError, RegistrationStatus};
/// 单个登记步骤及原始传输错误；明确拒绝没有伪造的普通事务ResultCode。
/// 对应 Java: AbstractIdentifyResponse及登记调用错误职责。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationAttempt {
    /// 接受/拒绝/本地失败/未知结果/尚未开始。
    pub status: RegistrationStatus,
    /// 仅传输失败时存在，保留代际/请求ID及写入前后分类。
    pub error: Option<CallError>,
}
impl RegistrationAttempt {
    /// 返回尚未尝试的步骤，不包含伪造错误或远端确认。
    pub(crate) fn pending() -> Self {
        Self {
            status: RegistrationStatus::NotAttempted,
            error: None,
        }
    }
    /// 参数是传输调用错误，返回保留未知/本地语义的步骤记录。
    pub(crate) fn failed(error: CallError) -> Self {
        Self {
            status: if error.is_unknown_outcome() {
                RegistrationStatus::UnknownOutcome
            } else {
                RegistrationStatus::LocalFailureBeforeSend
            },
            error: Some(error),
        }
    }
}
