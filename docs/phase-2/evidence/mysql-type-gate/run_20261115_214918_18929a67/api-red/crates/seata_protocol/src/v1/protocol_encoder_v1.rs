use super::frame_limits::FrameLimits;
use super::head_map_serializer::HeadMapSerializer;
use crate::codec::WireWriter;
use crate::protocol::RpcMessage;
use crate::{CompressorType, PayloadCodec, ProtocolError, RequestType, SerializerType};

/// 从字段对象生成完整V1帧；不吞掉编码错误，不接触网络或事务状态。
/// 对应 Java: org.apache.seata.core.rpc.netty.v1.ProtocolEncoderV1
#[derive(Debug, Clone, Copy)]
pub struct ProtocolEncoderV1 {
    limits: FrameLimits,
}
impl ProtocolEncoderV1 {
    /// 参数为已校验预算；返回无连接状态的完整帧编码器。
    pub const fn new(limits: FrameLimits) -> Self {
        Self { limits }
    }
    /// 返回此实例的显式帧/头/载荷预算。
    pub const fn limits(&self) -> FrameLimits {
        self.limits
    }
    /// 参数为完整RPC字段对象；返回新帧字节，任何字段/预算错误均不返回部分帧。
    pub fn encode(&self, message: &RpcMessage) -> Result<Vec<u8>, ProtocolError> {
        if message.codec != SerializerType::Seata {
            return Err(ProtocolError::UnsupportedSerializer {
                code: message.codec.to_java_v1(),
            });
        }
        if message.compressor != CompressorType::None {
            return Err(ProtocolError::UnsupportedCompressor {
                code: message.compressor.to_java_v1(),
            });
        }
        let is_heartbeat = matches!(
            message.message_type,
            RequestType::HeartbeatRequest | RequestType::HeartbeatResponse
        );
        if is_heartbeat != message.body.is_none() {
            return Err(ProtocolError::InvalidValue {
                field: "body",
                reason: "heartbeat requires None; ordinary frame requires Some message",
            });
        }
        let headers = HeadMapSerializer::encode(
            &message.head_map,
            self.limits.maximum_head_length - 16,
            self.limits.maximum_header_entries,
        )?;
        let head = 16 + headers.len();
        let remaining = self.limits.maximum_frame_length - head;
        let body = if let Some(value) = &message.body {
            if remaining < 2 {
                return Err(ProtocolError::LimitExceeded {
                    limit: "frame bytes",
                    maximum: self.limits.maximum_frame_length,
                    actual: head + 2,
                });
            }
            let mut payload_limits = self.limits.payload_limits;
            payload_limits.maximum_payload_length =
                payload_limits.maximum_payload_length.min(remaining);
            PayloadCodec::with_limits(payload_limits).encode(value)?
        } else {
            Vec::new()
        };
        let full = head
            .checked_add(body.len())
            .ok_or(ProtocolError::InvalidFrame {
                reason: "frame length overflow",
            })?;
        let mut writer = WireWriter::new(self.limits.maximum_frame_length);
        writer.write_bytes(&[0xda, 0xda, 1])?;
        // 配置已保证full在Java int范围，head在short范围；仍通过checked转换明确验证。
        writer.i32(
            i32::try_from(full).map_err(|_| ProtocolError::InvalidFrame {
                reason: "full length exceeds Java int",
            })?,
        )?;
        writer.i16(
            i16::try_from(head).map_err(|_| ProtocolError::InvalidFrame {
                reason: "head length exceeds Java short",
            })?,
        )?;
        writer.u8(message.message_type.to_java_v1())?;
        writer.u8(message.codec.to_java_v1())?;
        writer.u8(message.compressor.to_java_v1())?;
        writer.i32(message.id)?;
        writer.write_bytes(&headers)?;
        writer.write_bytes(&body)?;
        Ok(writer.into_bytes())
    }
}
