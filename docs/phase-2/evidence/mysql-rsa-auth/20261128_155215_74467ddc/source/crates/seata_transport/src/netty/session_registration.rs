use crate::{
    CallError, ConnectionHandle, RegistrationAttempt, RegistrationIdentity, RegistrationRole,
    RegistrationStatus, ResourceRegistrationResult, ResourceRegistry, SessionRegistrationReport,
    TransportError,
};
use seata_protocol::protocol::{RegisterRMRequest, RegisterTMRequest};
use seata_protocol::Message;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::Instant;
/// 原生身份/缓存资源初始化策略，所有新代际重新执行，绝不重发事务或BranchRegister。
/// 对应 Java: Tm/RmNettyRemotingClient注册及RegisterCachedResources职责。
#[derive(Debug, Clone)]
pub struct SessionRegistration {
    identity: Arc<RegistrationIdentity>,
    pub(crate) resources: ResourceRegistry,
    role: RegistrationRole,
    timeout: Duration,
}

#[cfg(test)]
#[path = "session_registration_tests.rs"]
mod tests;
impl SessionRegistration {
    /// 参数为显式TM身份和整轮登记预算；返回无资源步骤的TM策略。
    pub fn tm(identity: RegistrationIdentity, timeout: Duration) -> Result<Self, TransportError> {
        Self::new(
            identity,
            ResourceRegistry::new(1, 1)?,
            RegistrationRole::Tm,
            timeout,
        )
    }
    /// 参数为显式RM身份、共享资源缓存和整轮预算；返回每新会话全量核对的RM策略。
    pub fn rm(
        identity: RegistrationIdentity,
        resources: ResourceRegistry,
        timeout: Duration,
    ) -> Result<Self, TransportError> {
        Self::new(identity, resources, RegistrationRole::Rm, timeout)
    }
    fn new(
        identity: RegistrationIdentity,
        resources: ResourceRegistry,
        role: RegistrationRole,
        timeout: Duration,
    ) -> Result<Self, TransportError> {
        if timeout.is_zero() || std::time::Instant::now().checked_add(timeout).is_none() {
            return Err(TransportError::InvalidConfiguration {
                reason: "positive finite registration deadline required",
            });
        }
        Ok(Self {
            identity: Arc::new(identity),
            resources,
            role,
            timeout,
        })
    }
    /// 返回当前权威资源修订；毒锁错误不能被视为旧计划仍然有效。
    pub(crate) fn revision(&self) -> Result<u64, TransportError> {
        Ok(self.resources.snapshot()?.revision())
    }
    /// 返回初始化计划变更通知，先订阅后取快照，避免错过动态资源加入。
    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.resources.subscribe()
    }
    fn request(&self, resource: Option<&str>) -> Message {
        match self.role {
            RegistrationRole::Tm => Message::RegisterTMRequest(RegisterTMRequest {
                version: self.identity.version.clone(),
                application_id: self.identity.application_id.clone(),
                transaction_service_group: self.identity.transaction_service_group.clone(),
                extra_data: self.identity.extra_data.clone().unwrap_or_default(),
            }),
            RegistrationRole::Rm => Message::RegisterRMRequest(RegisterRMRequest {
                version: self.identity.version.clone(),
                application_id: Some(self.identity.application_id.clone()),
                transaction_service_group: self.identity.transaction_service_group.clone(),
                extra_data: self.identity.extra_data.clone(),
                resource_ids: resource.map(str::to_owned),
            }),
        }
    }
    async fn attempt(
        &self,
        handle: &ConnectionHandle,
        resource: Option<&str>,
        deadline: Instant,
    ) -> RegistrationAttempt {
        match handle.request_until(self.request(resource), deadline).await {
            Err(error) => RegistrationAttempt::failed(error),
            Ok(frame) => {
                let identified = match (self.role, frame.body) {
                    (RegistrationRole::Tm, Some(Message::RegisterTMResponse(value))) => {
                        value.identified
                    }
                    (RegistrationRole::Rm, Some(Message::RegisterRMResponse(value))) => {
                        value.identified
                    }
                    _ => {
                        let reason = TransportError::UnexpectedResponse {
                            reason: "registration response family differs",
                        };
                        handle.core.terminate(reason.clone());
                        return RegistrationAttempt::failed(CallError::request(
                            handle.generation(),
                            frame.id,
                            true,
                            reason,
                        ));
                    }
                };
                RegistrationAttempt {
                    status: if identified {
                        RegistrationStatus::Accepted
                    } else {
                        RegistrationStatus::Rejected
                    },
                    error: None,
                }
            }
        }
    }
    /// 参数为此代际的raw句柄；返回同一资源快照/绝对期限下的全步骤报告，后续未尝试项保持显式。
    pub(crate) async fn initialize(
        &self,
        handle: &ConnectionHandle,
    ) -> Result<SessionRegistrationReport, TransportError> {
        let snapshot = self.resources.snapshot()?;
        let deadline = Instant::now().checked_add(self.timeout).ok_or(
            TransportError::InvalidConfiguration {
                reason: "registration deadline overflow",
            },
        )?;
        let mut report = SessionRegistrationReport {
            generation: handle.generation(),
            registry_revision: snapshot.revision(),
            role: self.role,
            identity: RegistrationAttempt::pending(),
            resources: snapshot
                .resources()
                .iter()
                .cloned()
                .map(|resource| ResourceRegistrationResult {
                    resource,
                    attempt: RegistrationAttempt::pending(),
                })
                .collect(),
        };
        if self.role == RegistrationRole::Tm || report.resources.is_empty() {
            report.identity = self.attempt(handle, None, deadline).await;
        } else {
            // RM认证扩展可同时检查资源；首个必需资源也承担初始身份登记，不强加空资源握手。
            for (index, entry) in report.resources.iter_mut().enumerate() {
                entry.attempt = self
                    .attempt(handle, Some(entry.resource.resource_id()), deadline)
                    .await;
                if index == 0 {
                    report.identity = entry.attempt.clone();
                }
                if entry.attempt.status != RegistrationStatus::Accepted {
                    break;
                }
            }
        }
        Ok(report)
    }
}
