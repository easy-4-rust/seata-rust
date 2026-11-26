use crate::{ResourceCatalog, RmCallError, RmOperation};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::{
    BranchRegisterRequest, BranchRegisterResponse, BranchReportRequest, BranchReportResponse,
    GlobalLockQueryRequest, GlobalLockQueryResponse,
};
use seata_protocol::{Message, ResultCode};
use seata_transport::{PoolHandle, ResourceKey};
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
        let reply = self
            .pool
            .request(request.clone(), self.timeout)
            .await
            .map_err(|source| RmCallError::from_call(operation, request.clone(), source))?;
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
        if false {
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
