use crate::{ResourceCatalog, RmCallError, RmOperation};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::{
    BranchRegisterRequest, BranchRegisterResponse, BranchReportRequest, BranchReportResponse,
    GlobalLockQueryRequest, GlobalLockQueryResponse,
};
use seata_protocol::{Message, ResultCode};
use seata_transport::{PoolHandle, ResourceKey};
use sha2::{Digest, Sha256};
use std::time::Duration;

/// 可克隆的已注册RM调用入口；网络所有权属于ResourceManager，每个业务请求只发送一次。
/// 对应 Java: AbstractResourceManager的三类出站控制调用及RmNettyRemotingClient。
#[derive(Debug, Clone)]
pub struct ResourceClient {
    pub(crate) pool: PoolHandle,
    pub(crate) catalog: ResourceCatalog,
    pub(crate) timeout: Duration,
}
impl ResourceClient {
    /// 参数为原始分支申请；仅在单次注册获得匹配成功响应与真实正编号后返回不可公开构造的回执。
    /// 错误沿用 ADR-0008；响应丢失不合成身份、不重发，原低层 branch_register 接口保持兼容。
    pub async fn branch_register_with_receipt(
        &self,
        request: BranchRegisterRequest,
    ) -> Result<crate::RegisteredBranch, RmCallError> {
        self.validate(
            request.xid.as_deref(),
            request.resource_id.as_deref(),
            request.branch_type,
            request.application_data.as_deref(),
            request.lock_key.as_deref(),
            true,
        )?;
        let original = request.clone();
        let response = self.branch_register(request).await?;
        let identity = crate::BranchIdentity::from_fields(
            original.xid.as_deref().ok_or(RmCallError::Invalid {
                reason: "XID required",
            })?,
            response.branch_id,
            original.branch_type,
            original
                .resource_id
                .as_deref()
                .ok_or(RmCallError::Invalid {
                    reason: "resource required",
                })?,
            original.application_data.as_deref(),
        )
        .map_err(|_| RmCallError::ProtocolFailure {
            operation: RmOperation::BranchRegister,
            request: Box::new(Message::BranchRegisterRequest(original.clone())),
            response: None,
            may_have_effect: true,
            reason: "issued registration identity cannot be represented",
            source: None,
        })?;
        Ok(crate::RegisteredBranch::issued(
            identity,
            original.lock_key,
            response,
        ))
    }
    /// 参数为完整原始分支申请；返回TC真实正i64编号及响应前缀；未知不合成编号、不重发。
    pub async fn branch_register(
        &self,
        request: BranchRegisterRequest,
    ) -> Result<BranchRegisterResponse, RmCallError> {
        self.validate(
            request.xid.as_deref(),
            request.resource_id.as_deref(),
            request.branch_type,
            request.application_data.as_deref(),
            request.lock_key.as_deref(),
            true,
        )?;
        let body = Message::BranchRegisterRequest(request);
        let reply = self.call(RmOperation::BranchRegister, body.clone()).await?;
        match reply.body.as_ref() {
            Some(Message::BranchRegisterResponse(value)) => Ok(value.clone()),
            _ => Err(unexpected_response(
                RmOperation::BranchRegister,
                body,
                reply,
            )),
        }
    }
    /// 参数为完整分支状态报告；允许Java原生可选资源字段为None，不自行填充；只发送一次。
    pub async fn branch_report(
        &self,
        request: BranchReportRequest,
    ) -> Result<BranchReportResponse, RmCallError> {
        self.validate(
            request.xid.as_deref(),
            request.resource_id.as_deref(),
            request.branch_type,
            request.application_data.as_deref(),
            None,
            false,
        )?;
        if request.branch_id <= 0 {
            return Err(RmCallError::Invalid {
                reason: "positive branch ID required",
            });
        }
        let body = Message::BranchReportRequest(request);
        let reply = self.call(RmOperation::BranchReport, body.clone()).await?;
        match reply.body.as_ref() {
            Some(Message::BranchReportResponse(value)) => Ok(value.clone()),
            _ => Err(unexpected_response(RmOperation::BranchReport, body, reply)),
        }
    }
    /// 参数为普通锁查询；成功时保留lockable=false，Failed或协议/传输异常独立返回。
    pub async fn lock_query(
        &self,
        request: GlobalLockQueryRequest,
    ) -> Result<GlobalLockQueryResponse, RmCallError> {
        self.validate(
            request.xid.as_deref(),
            request.resource_id.as_deref(),
            request.branch_type,
            request.application_data.as_deref(),
            request.lock_key.as_deref(),
            true,
        )?;
        let body = Message::GlobalLockQueryRequest(request);
        let reply = self.call(RmOperation::LockQuery, body.clone()).await?;
        match reply.body.as_ref() {
            Some(Message::GlobalLockQueryResponse(value)) => Ok(value.clone()),
            _ => Err(unexpected_response(RmOperation::LockQuery, body, reply)),
        }
    }
    fn validate(
        &self,
        xid: Option<&str>,
        resource: Option<&str>,
        kind: seata_protocol::BranchType,
        app: Option<&str>,
        lock: Option<&str>,
        required: bool,
    ) -> Result<(), RmCallError> {
        if !xid.is_some_and(|v| {
            !v.is_empty() && v.len() <= 32767 && v.trim() == v && !v.chars().any(char::is_control)
        }) || [app, lock]
            .into_iter()
            .flatten()
            .any(|v| v.is_empty() || v.len() > 1_048_576)
        {
            return Err(RmCallError::Invalid {
                reason: "invalid or ambiguous XID/application data/lock key",
            });
        }
        if required && resource.is_none() {
            return Err(RmCallError::Invalid {
                reason: "resource required",
            });
        }
        if let Some(value) = resource {
            if value.len() > 32767 {
                return Err(RmCallError::Invalid {
                    reason: "resource exceeds wire bound",
                });
            }
            let key = ResourceKey::new(kind, value.into()).map_err(|_| RmCallError::Invalid {
                reason: "invalid resource key",
            })?;
            self.catalog
                .lookup(&key)
                .map_err(|_| RmCallError::Invalid {
                    reason: "resource handler not registered",
                })?;
        }
        Ok(())
    }
    async fn call(
        &self,
        operation: RmOperation,
        request: Message,
    ) -> Result<RpcMessage, RmCallError> {
        let (xid, resource, branch_id) = match &request {
            Message::BranchRegisterRequest(value) => {
                (value.xid.as_deref(), value.resource_id.as_deref(), None)
            }
            Message::BranchReportRequest(value) => (
                value.xid.as_deref(),
                value.resource_id.as_deref(),
                Some(value.branch_id),
            ),
            Message::GlobalLockQueryRequest(value) => {
                (value.xid.as_deref(), value.resource_id.as_deref(), None)
            }
            _ => (None, None, None),
        };
        let reply = match self.pool.request(request.clone(), self.timeout).await {
            Ok(reply) => {
                // 仅记录收到报文；ResultCode和分支编号仍由下文校验。
                tracing::info!(target: "seata_rm::rpc", operation = ?operation, xid = ?xid, branch_id = ?branch_id, resource_sha256 = ?resource.map(resource_sha256), request_id = reply.id, outcome = "response", "rm_rpc_outcome");
                reply
            }
            Err(source) => {
                let request_id = source.request_id();
                let outcome = if source.is_unknown_outcome() {
                    "unknown"
                } else {
                    "not_sent"
                };
                // 资源原值、application_data、锁键及错误Debug均不得进入事件。
                tracing::warn!(target: "seata_rm::rpc", operation = ?operation, xid = ?xid, branch_id = ?branch_id, resource_sha256 = ?resource.map(resource_sha256), request_id = ?request_id, outcome, "rm_rpc_outcome");
                return Err(RmCallError::from_call(operation, request, source));
            }
        };
        let prefix = match (&reply.body, operation) {
            (Some(Message::BranchRegisterResponse(v)), RmOperation::BranchRegister) => {
                Some(&v.response)
            }
            (Some(Message::BranchReportResponse(v)), RmOperation::BranchReport) => {
                Some(&v.response)
            }
            (Some(Message::GlobalLockQueryResponse(v)), RmOperation::LockQuery) => {
                Some(&v.response)
            }
            _ => None,
        };
        let Some(prefix) = prefix else {
            return Err(RmCallError::ProtocolFailure {
                operation,
                request: Box::new(request),
                response: Some(Box::new(reply)),
                may_have_effect: true,
                reason: "unexpected response family",
                source: None,
            });
        };
        if prefix.result_code == ResultCode::Failed {
            return Err(RmCallError::DefiniteRejected {
                operation,
                request: Box::new(request),
                response: Box::new(reply),
            });
        }
        if matches!(&reply.body,Some(Message::BranchRegisterResponse(v)) if v.branch_id<=0) {
            return Err(RmCallError::ProtocolFailure {
                operation,
                request: Box::new(request),
                response: Some(Box::new(reply)),
                may_have_effect: true,
                reason: "successful branch registration requires a positive issued ID",
                source: None,
            });
        }
        Ok(reply)
    }
}
fn resource_sha256(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn unexpected_response(
    operation: RmOperation,
    request: Message,
    response: RpcMessage,
) -> RmCallError {
    RmCallError::ProtocolFailure {
        operation,
        request: Box::new(request),
        response: Some(Box::new(response)),
        may_have_effect: true,
        reason: "unacceptable response family",
        source: None,
    }
}
