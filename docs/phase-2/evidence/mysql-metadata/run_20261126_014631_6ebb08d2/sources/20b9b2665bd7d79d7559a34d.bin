use crate::Xid;
use seata_protocol::TransactionExceptionCode;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::Instant;

/// Begin 成功后、业务开始前需要落盘的真实 TM 来源；对应 Java: DefaultGlobalTransaction#begin。
pub struct LauncherOrigin {
    pub(crate) xid: Xid,
    pub(crate) tc_endpoint: SocketAddr,
    pub(crate) application_id: String,
    pub(crate) service_group: String,
    pub(crate) begin_exception_code: TransactionExceptionCode,
    pub(crate) begin_extra_data: Option<String>,
    pub(crate) business_intent: String,
    pub(crate) remaining: Duration,
    pub(crate) captured_at: Instant,
    pub(crate) owner_instance_id: [u8; 32],
    pub(crate) owner_stop_scope: Option<[u8; 32]>,
}
impl LauncherOrigin {
    /// 返回 TC 原始 XID；持久层须同时保存原文与摘要。
    pub fn xid(&self) -> &Xid {
        &self.xid
    }
    /// 返回原登记的数值 TC 端点。
    pub const fn tc_endpoint(&self) -> SocketAddr {
        self.tc_endpoint
    }
    /// 返回原登记的应用标识。
    pub fn application_id(&self) -> &str {
        &self.application_id
    }
    /// 返回原登记的事务服务组。
    pub fn service_group(&self) -> &str {
        &self.service_group
    }
    /// 返回 Begin 响应的原异常码。
    pub const fn begin_exception_code(&self) -> TransactionExceptionCode {
        self.begin_exception_code
    }
    /// 返回 Begin 响应的原可选附加数据。
    pub fn begin_extra_data(&self) -> Option<&str> {
        self.begin_extra_data.as_deref()
    }
    /// 返回业务侧的幂等意图标识。
    pub fn business_intent(&self) -> &str {
        &self.business_intent
    }
    /// 返回签发此 Launcher 的管理器随机实例 ID；不是恢复授权。
    pub const fn owner_instance_id(&self) -> &[u8; 32] {
        &self.owner_instance_id
    }
    /// 返回部署配置的拥有者停止证明信任域。无配置时仅允许本机停止证明。
    pub fn owner_stop_scope(&self) -> Option<&[u8; 32]> {
        self.owner_stop_scope.as_ref()
    }
    /// 返回 Begin 到落盘前剩余的保守期限。
    pub fn remaining(&self) -> Duration {
        self.remaining.saturating_sub(self.captured_at.elapsed())
    }
}
