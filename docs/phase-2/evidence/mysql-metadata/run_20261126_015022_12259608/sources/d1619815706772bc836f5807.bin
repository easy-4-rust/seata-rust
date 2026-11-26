use crate::{
    CallError, ConnectionGeneration, RegistrationAttempt, RegistrationRole, RegistrationStatus,
    ResourceRegistrationResult, TransportError,
};
/// 特定代际/缓存修订的完整有界初始化报告；不能跨代际复用其成功记录。
/// 对应 Java: 会话打开登记及RegisterCachedResources完整结果职责。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRegistrationReport {
    /// 实际执行登记的物理代际。
    pub generation: ConnectionGeneration,
    /// 本次完整计划依据的资源缓存修订。
    pub registry_revision: u64,
    /// 本通道唯一角色。
    pub role: RegistrationRole,
    /// 身份登记步骤。
    pub identity: RegistrationAttempt,
    /// 所有所需资源的有序结果，包含未执行步骤。
    pub resources: Vec<ResourceRegistrationResult>,
}
impl SessionRegistrationReport {
    /// 返回此快照是否每个步骤均被接受；池还必须核对当前修订/代际，不能只调用本方法授权新版本资源。
    pub fn is_ready(&self) -> bool {
        self.identity.status == RegistrationStatus::Accepted
            && self
                .resources
                .iter()
                .all(|entry| entry.attempt.status == RegistrationStatus::Accepted)
    }
    /// 返回首次失败的基础原因；保留传输不确定性，绝不把未尝试/丢响应变成明确拒绝。
    pub(crate) fn failure_reason(&self) -> Option<TransportError> {
        for (index, attempt) in std::iter::once((None, &self.identity)).chain(
            self.resources
                .iter()
                .enumerate()
                .map(|(index, entry)| (Some(index), &entry.attempt)),
        ) {
            if attempt.status == RegistrationStatus::Rejected {
                return Some(TransportError::RegistrationRejected {
                    resource_index: index,
                });
            }
            if let Some(
                CallError::LocalFailureBeforeSend { reason, .. }
                | CallError::UnknownOutcome { reason, .. },
            ) = &attempt.error
            {
                return Some(reason.clone());
            }
        }
        if self.is_ready() {
            None
        } else {
            Some(TransportError::RegistrationIncomplete)
        }
    }
}
