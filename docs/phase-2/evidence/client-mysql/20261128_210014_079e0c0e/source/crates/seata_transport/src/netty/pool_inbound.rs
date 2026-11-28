use crate::ConnectionHandle;
use seata_protocol::protocol::RpcMessage;
use std::fmt;
/// 带原通道身份的入站请求；旧代际事件不能被错误地重绑定到新socket。
/// 对应 Java: ChannelHandlerContext与RpcMessage的入站分发组合职责。
pub struct PoolInbound {
    pub(crate) connection: ConnectionHandle,
    pub(crate) message: RpcMessage,
}
impl PoolInbound {
    /// 消费此原代际请求并回包；需要启用I/O/time的Tokio运行时，不从池重新取连接。
    /// 成功仅表示本地完整写入，不表示TC确认。
    pub async fn respond(
        self,
        body: seata_protocol::Message,
        timeout: std::time::Duration,
    ) -> Result<(), crate::CallError> {
        self.connection.respond(&self.message, body, timeout).await
    }
    /// 返回发起该消息的原始连接句柄借用，不自动迁移或重连。
    pub const fn connection(&self) -> &ConnectionHandle {
        &self.connection
    }
    /// 返回原始消息借用，保留其ID/字段。
    pub const fn message(&self) -> &RpcMessage {
        &self.message
    }
    /// 消费事件，返回原连接句柄和完整消息，供上层业务分发。
    pub fn into_parts(self) -> (ConnectionHandle, RpcMessage) {
        (self.connection, self.message)
    }
}
impl fmt::Debug for PoolInbound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolInbound")
            .field("generation", &self.connection.generation())
            .field("id", &self.message.id)
            .finish()
    }
}
