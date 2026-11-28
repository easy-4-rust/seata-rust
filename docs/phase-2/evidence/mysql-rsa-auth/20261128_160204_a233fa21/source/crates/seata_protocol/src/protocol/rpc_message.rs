use crate::{CompressorType, Message, RequestType, SerializerType};
use std::fmt;

/// 原生V1 RPC信封；请求编号保留Java signed-int，头字段保留顺序及null/空值。
/// 对应 Java: org.apache.seata.core.protocol.RpcMessage 的线缆字段；otherSideVersion不在线缆上。
#[derive(Clone, PartialEq, Eq)]
pub struct RpcMessage {
    /// 有符号请求编号，不借助负值表示错误或缺省。
    pub id: i32,
    /// 外层请求/响应/单向/心跳种类，不等同内层MessageType。
    pub message_type: RequestType,
    /// 序列化器域，首期仅Seata可执行。
    pub codec: SerializerType,
    /// 压缩器域，首期仅None可执行。
    pub compressor: CompressorType,
    /// 原始有序头字段；key不能为空指针但可为空串，value的None与Some空串分别编码-1/0。
    /// 重复key由严格编解码入口拒绝，避免Java Map覆盖造成歧义。
    pub head_map: Vec<(String, Option<String>)>,
    /// 心跳必须None且线缆body为空；其余类型必须Some完整消息。
    pub body: Option<Message>,
}
impl fmt::Debug for RpcMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RpcMessage")
            .field("id", &self.id)
            .field("message_type", &self.message_type)
            .field("codec", &self.codec)
            .field("compressor", &self.compressor)
            .field("header_entries", &self.head_map.len())
            .field("body", &self.body)
            .finish()
    }
}
