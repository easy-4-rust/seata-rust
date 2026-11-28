use crate::codec::{WireReader, WireWriter};
use crate::protocol::{MergeResultMessage, MergedWarpMessage};
use crate::{Message, MessageType, PayloadLimits, ProtocolError};

/// 严格V1载荷编解码器；显式限制字节/节点/批量/层数，不承担帧或业务执行。
/// 对应 Java: org.apache.seata.serializer.seata.SeataSerializer 的V1载荷职责。
#[derive(Debug, Clone, Copy)]
pub struct PayloadCodec {
    limits: PayloadLimits,
}
impl PayloadCodec {
    /// 参数为完整载荷字节预算；返回普通消息实例，聚合默认禁用以保留原资源边界。
    /// 聚合使用with_limits显式配置；不足两字节类型前缀时返回错误。
    pub fn new(maximum_payload_length: usize) -> Result<Self, ProtocolError> {
        Ok(Self::with_limits(PayloadLimits::new(
            maximum_payload_length,
            0,
            1,
            0,
        )?))
    }
    /// 参数为已校验全部预算，返回支持批准27类型的实例；超预算仍明确拒绝。
    pub const fn with_limits(limits: PayloadLimits) -> Self {
        Self { limits }
    }
    /// 返回当前实例的完整资源预算。
    pub const fn limits(&self) -> PayloadLimits {
        self.limits
    }
    /// 返回此实例的显式载荷字节预算。
    pub const fn maximum_payload_length(&self) -> usize {
        self.limits.maximum_payload_length
    }
    /// 返回实现能力；不表示某个对象符合实例字节/节点/层数预算。
    pub const fn supports(message_type: MessageType) -> bool {
        message_type.in_v1_payload_allowlist()
    }
    /// 输入完整带类型前缀payload，返回完整树；截断、尾随、非法字段及超预算均拒绝。
    pub fn decode(&self, input: &[u8]) -> Result<Message, ProtocolError> {
        let mut reader = WireReader::new(input, self.limits.maximum_payload_length)?;
        let mut remaining_nodes = self.limits.maximum_total_nodes;
        let message = self.read_node(&mut reader, &mut remaining_nodes, 0)?;
        reader.finish()?;
        Ok(message)
    }
    /// 从对象字段生成新的带类型前缀payload；错误均丢弃部分输出，不缓存原输入。
    pub fn encode(&self, message: &Message) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new(self.limits.maximum_payload_length);
        let mut remaining_nodes = self.limits.maximum_total_nodes;
        self.write_node(message, &mut writer, &mut remaining_nodes, 0)?;
        Ok(writer.into_bytes())
    }
    fn consume_node(&self, remaining: &mut usize) -> Result<(), ProtocolError> {
        if *remaining == 0 {
            return Err(ProtocolError::LimitExceeded {
                limit: "payload nodes",
                maximum: self.limits.maximum_total_nodes,
                actual: self.limits.maximum_total_nodes.saturating_add(1),
            });
        }
        *remaining -= 1;
        Ok(())
    }
    fn batch(&self, count: usize, depth: usize, remaining: usize) -> Result<(), ProtocolError> {
        for (limit, maximum, actual) in [
            ("merge depth", self.limits.maximum_nesting_depth, depth),
            ("batch children", self.limits.maximum_batch_children, count),
            (
                "payload nodes",
                self.limits.maximum_total_nodes,
                (self.limits.maximum_total_nodes - remaining).saturating_add(count),
            ),
        ] {
            if actual > maximum {
                return Err(ProtocolError::LimitExceeded {
                    limit,
                    maximum,
                    actual,
                });
            }
        }
        Ok(())
    }
    fn read_node(
        &self,
        reader: &mut WireReader<'_>,
        remaining: &mut usize,
        depth: usize,
    ) -> Result<Message, ProtocolError> {
        self.consume_node(remaining)?;
        let kind = MessageType::from_java_v1(reader.u16()?)?;
        let is_result = match kind {
            MessageType::MergedWarp => false,
            MessageType::MergeResult => true,
            _ => return Message::read(kind, reader),
        };
        let length = reader.i32()?;
        if length < 2 {
            return Err(ProtocolError::InvalidLength {
                field: "merge content",
                length: i64::from(length),
            });
        }
        // 子游标被长度封闭，子消息和编号不能读取到下一条兄弟消息或帧。
        let content = reader.take(length as usize, "merge content")?;
        let mut body = WireReader::new(content, content.len())?;
        let signed_count = body.i16()?;
        if signed_count < 0 {
            return Err(ProtocolError::InvalidLength {
                field: "merge child count",
                length: i64::from(signed_count),
            });
        }
        let count = signed_count as usize;
        self.batch(count, depth + 1, *remaining)?;
        if count > body.remaining() / 2 {
            return Err(ProtocolError::Truncated {
                field: "merge child types",
                needed: count * 2,
                remaining: body.remaining(),
            });
        }
        // 数量已受预算和输入限制；逐条增长，不依赖恶意count预分配。
        let mut msgs = Vec::new();
        for _ in 0..count {
            let child = self.read_node(&mut body, remaining, depth + 1)?;
            if is_result && !child.is_result() {
                return Err(ProtocolError::InvalidValue {
                    field: "msgs",
                    reason: "merged results require AbstractResultMessage children",
                });
            }
            msgs.push(child);
        }
        if is_result {
            body.finish()?;
            Ok(Message::MergeResultMessage(MergeResultMessage {
                msgs: Some(msgs),
            }))
        } else {
            let mut msg_ids = Vec::new();
            if body.remaining() != 0 {
                if body.remaining() != count * 4 {
                    return Err(ProtocolError::InvalidValue {
                        field: "msg_ids",
                        reason: "IDs must be absent or match every child",
                    });
                }
                for _ in 0..count {
                    msg_ids.push(body.i32()?);
                }
            }
            body.finish()?;
            Ok(Message::MergedWarpMessage(MergedWarpMessage {
                msgs,
                msg_ids,
            }))
        }
    }
    fn write_node(
        &self,
        message: &Message,
        writer: &mut WireWriter,
        remaining: &mut usize,
        depth: usize,
    ) -> Result<(), ProtocolError> {
        self.consume_node(remaining)?;
        writer.u16(message.message_type().to_java_v1())?;
        let (msgs, ids): (&[Message], &[i32]) = match message {
            Message::MergedWarpMessage(value) => {
                value.validate_ids()?;
                (&value.msgs, &value.msg_ids)
            }
            Message::MergeResultMessage(value) => (value.initialized_msgs()?, &[]),
            _ => return message.write(writer),
        };
        self.batch(msgs.len(), depth + 1, *remaining)?;
        let count = i16::try_from(msgs.len()).map_err(|_| ProtocolError::InvalidLength {
            field: "merge child count",
            length: i64::try_from(msgs.len()).unwrap_or(i64::MAX),
        })?;
        let offset = writer.len();
        writer.i32(0)?;
        writer.i16(count)?;
        for child in msgs {
            if matches!(message, Message::MergeResultMessage(_)) && !child.is_result() {
                return Err(ProtocolError::InvalidValue {
                    field: "msgs",
                    reason: "merged results require AbstractResultMessage children",
                });
            }
            self.write_node(child, writer, remaining, depth + 1)?;
        }
        for id in ids {
            writer.i32(*id)?;
        }
        // 同一输出回填，避免每一层复制整个子树造成额外内存放大。
        let content_length = writer.len() - offset - 4;
        let length = i32::try_from(content_length).map_err(|_| ProtocolError::InvalidLength {
            field: "merge content",
            length: i64::try_from(content_length).unwrap_or(i64::MAX),
        })?;
        writer.replace_i32(offset, length)
    }
}
