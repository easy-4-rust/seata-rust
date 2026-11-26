use seata_protocol::TransactionExceptionCode;
use seata_tm::{TmError, TmOperation, TransactionManager, TransactionOutcome, Xid};
use std::net::SocketAddr;

/// 已逐字节核验的 008 TM 来源只读快照；不携带 Launcher 结束权。
/// 对应 Java: DefaultGlobalTransaction 的原 XID/登记身份与结束状态观察。
pub struct MySqlLauncherSnapshot {
    pub(crate) xid: Xid,
    pub(crate) tc_endpoint: SocketAddr,
    pub(crate) peer_trust_binding: Option<[u8; 32]>,
    pub(crate) application_id: String,
    pub(crate) service_group: String,
    pub(crate) begin_exception_code: TransactionExceptionCode,
    pub(crate) begin_extra_data: Option<String>,
    pub(crate) business_intent: String,
    pub(crate) owner_epoch: u64,
    pub(crate) owner_instance_id: [u8; 32],
    pub(crate) owner_stop_scope: Option<[u8; 32]>,
    pub(crate) end_operation: Option<TmOperation>,
    pub(crate) send_started: bool,
    pub(crate) deadline_passed: bool,
}
impl MySqlLauncherSnapshot {
    /// 使用给定 TM 和业务意图查询来源的原 TC，返回一次原始状态响应。
    /// 使用快照读取时的数据库期限判定。结果仅用于观察，Finished 保留原值。
    /// 对应 Java: DefaultGlobalTransaction#getStatus，额外核对持久来源身份。
    pub async fn query_original_tc(
        &self,
        manager: &TransactionManager,
        business_intent: &str,
    ) -> Result<TransactionOutcome, TmError> {
        if self.peer_trust_binding.as_ref() != manager.peer_trust_binding() {
            return Err(TmError::IllegalState {
                xid: Some(self.xid.clone()),
                reason: "TM transport trust does not match launcher origin",
            });
        }
        if self.business_intent != business_intent {
            return Err(TmError::IllegalState {
                xid: Some(self.xid.clone()),
                reason: "business intent does not match launcher origin",
            });
        }
        if self.deadline_passed {
            return Err(TmError::TransactionDeadlineExceeded {
                xid: self.xid.clone(),
            });
        }
        manager
            .get_status_at_origin(
                &self.xid,
                self.tc_endpoint,
                &self.application_id,
                &self.service_group,
            )
            .await
    }
    /// 返回已和请求逐字节匹配的原 XID。
    pub fn xid(&self) -> &Xid {
        &self.xid
    }
    /// 返回来源记录中的原 TC 数值端点。
    pub const fn tc_endpoint(&self) -> SocketAddr {
        self.tc_endpoint
    }
    /// 无参数；返回来源的 TC 信任摘要；旧明文来源为 None，不构成恢复权限。
    pub fn peer_trust_binding(&self) -> Option<&[u8; 32]> {
        self.peer_trust_binding.as_ref()
    }
    /// 返回原 TM 登记应用标识。
    pub fn application_id(&self) -> &str {
        &self.application_id
    }
    /// 返回原 TM 登记服务组。
    pub fn service_group(&self) -> &str {
        &self.service_group
    }
    /// 返回原 Begin 响应异常码。
    pub const fn begin_exception_code(&self) -> TransactionExceptionCode {
        self.begin_exception_code
    }
    /// 返回原 Begin 可选附加数据。
    pub fn begin_extra_data(&self) -> Option<&str> {
        self.begin_extra_data.as_deref()
    }
    /// 返回业务幂等意图标识。
    pub fn business_intent(&self) -> &str {
        &self.business_intent
    }
    /// 返回持久拥有者代际；此值不构成接管许可。
    pub const fn owner_epoch(&self) -> u64 {
        self.owner_epoch
    }
    /// 返回签发原 Launcher 的 TM 随机实例 ID；不构成进程停止证明。
    pub const fn owner_instance_id(&self) -> &[u8; 32] {
        &self.owner_instance_id
    }
    /// 返回来源中拥有者绑定的停止证明信任域。历史无域来源返回 None。
    pub fn owner_stop_scope(&self) -> Option<&[u8; 32]> {
        self.owner_stop_scope.as_ref()
    }
    /// 返回已确认的结束意图，无意图为 None。
    pub const fn end_operation(&self) -> Option<TmOperation> {
        self.end_operation
    }
    /// 返回不可清除的结束发送门闩状态。
    pub const fn send_started(&self) -> bool {
        self.send_started
    }
    /// 返回同一 MySQL 服务端时钟判断的来源期限是否已过。
    pub const fn deadline_passed(&self) -> bool {
        self.deadline_passed
    }
}
