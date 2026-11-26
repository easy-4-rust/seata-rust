use crate::{Message, ProtocolError};

/// 聚合结果；只允许Java AbstractResultMessage子类，保持原请求顺序。
/// 对应 Java: org.apache.seata.core.protocol.MergeResultMessage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResultMessage {
    /// Java未设置的数组为None；线缆上只能编码已设置的数组，Some空列表合法。
    pub msgs: Option<Vec<Message>>,
}

impl MergeResultMessage {
    /// 返回已设置数组的借用；不遍历子消息，由codec在数量预算检查后逐个校验类型。
    pub(crate) fn initialized_msgs(&self) -> Result<&[Message], ProtocolError> {
        let msgs = self.msgs.as_deref().ok_or(ProtocolError::InvalidValue {
            field: "msgs",
            reason: "result array must be initialized",
        })?;
        Ok(msgs)
    }
}
