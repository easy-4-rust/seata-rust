use crate::{Message, ProtocolError};

/// 保持原序的聚合消息及可选的完整子请求编号列表。
/// 对应 Java: org.apache.seata.core.protocol.MergedWarpMessage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedWarpMessage {
    /// 有序子消息；可含普通请求、响应或嵌套聚合。
    pub msgs: Vec<Message>,
    /// 为空表示整批省略编号，否则必须与msgs逐个对应；保留有符号int。
    pub msg_ids: Vec<i32>,
}

impl MergedWarpMessage {
    /// 验证对象的编号关系；参数来自自身字段，无返回数据，形状错误返回协议错误。
    pub(crate) fn validate_ids(&self) -> Result<(), ProtocolError> {
        if !self.msg_ids.is_empty() && self.msg_ids.len() != self.msgs.len() {
            return Err(ProtocolError::InvalidValue {
                field: "msg_ids",
                reason: "IDs must be absent or match every child",
            });
        }
        Ok(())
    }
}
