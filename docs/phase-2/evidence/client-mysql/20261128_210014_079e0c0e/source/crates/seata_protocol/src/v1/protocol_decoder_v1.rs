use super::frame_limits::FrameLimits;
use super::head_map_serializer::HeadMapSerializer;
use crate::codec::WireReader;
use crate::protocol::RpcMessage;
use crate::{CompressorType, PayloadCodec, ProtocolError, RequestType, SerializerType};

/// 完整V1帧严格解码；流分片/连续帧拆分由连接组件负责，本入口一次只接受一帧。
/// 对应 Java: org.apache.seata.core.rpc.netty.v1.ProtocolDecoderV1
#[derive(Debug, Clone, Copy)]
pub struct ProtocolDecoderV1 {
    limits: FrameLimits,
}
impl ProtocolDecoderV1 {
    /// 参数为已校验预算；返回无连接状态的帧解码器。
    pub const fn new(limits: FrameLimits) -> Self {
        Self { limits }
    }
    /// 返回此实例的显式帧/头/载荷预算。
    pub const fn limits(&self) -> FrameLimits {
        self.limits
    }
    /// 参数为任意长度的流前缀；检查已到齐的固定头字段，完整有效固定头返回整帧长度。
    /// 不足16字节但仍可能有效时返回None；错误magic/版本/长度/能力尽早返回错误。
    /// 只检查固定头，不解码HeadMap/正文，也不把后续连续帧当尾随数据。
    pub fn frame_length(&self, prefix: &[u8]) -> Result<Option<usize>, ProtocolError> {
        Ok(self.header(prefix)?.map(|(full, _, _)| full))
    }

    // 私有阶段对象只在本实现内部流转，普通消息body尚未设置时绝不返回给调用方。
    fn header(&self, prefix: &[u8]) -> Result<Option<(usize, usize, RpcMessage)>, ProtocolError> {
        for (index, expected) in [(0, 0xda), (1, 0xda), (2, 1)] {
            if let Some(actual) = prefix.get(index) {
                if *actual != expected {
                    return Err(ProtocolError::InvalidFrame {
                        reason: if index == 2 {
                            "only protocol V1 is supported"
                        } else {
                            "wrong magic"
                        },
                    });
                }
            }
        }
        if prefix.len() < 7 {
            return Ok(None);
        }
        let full = u32::from_be_bytes([prefix[3], prefix[4], prefix[5], prefix[6]]) as usize;
        if full < 16 {
            return Err(ProtocolError::InvalidFrame {
                reason: "full length is smaller than fixed header",
            });
        }
        if full > self.limits.maximum_frame_length {
            return Err(ProtocolError::LimitExceeded {
                limit: "frame bytes",
                maximum: self.limits.maximum_frame_length,
                actual: full,
            });
        }
        if prefix.len() < 9 {
            return Ok(None);
        }
        let signed_head = i16::from_be_bytes([prefix[7], prefix[8]]);
        if signed_head < 16 {
            return Err(ProtocolError::InvalidFrame {
                reason: "head length is outside signed-short V1 range",
            });
        }
        let head = signed_head as usize;
        if head > full {
            return Err(ProtocolError::InvalidFrame {
                reason: "head length exceeds full length",
            });
        }
        if head > self.limits.maximum_head_length {
            return Err(ProtocolError::LimitExceeded {
                limit: "head bytes",
                maximum: self.limits.maximum_head_length,
                actual: head,
            });
        }
        if prefix.len() < 10 {
            return Ok(None);
        }
        let message_type = RequestType::from_java_v1(prefix[9])?;
        if prefix.len() < 11 {
            return Ok(None);
        }
        let codec = SerializerType::from_java_v1(prefix[10])?;
        if codec != SerializerType::Seata {
            return Err(ProtocolError::UnsupportedSerializer {
                code: codec.to_java_v1(),
            });
        }
        if prefix.len() < 12 {
            return Ok(None);
        }
        let compressor = CompressorType::from_java_v1(prefix[11])?;
        if compressor != CompressorType::None {
            return Err(ProtocolError::UnsupportedCompressor {
                code: compressor.to_java_v1(),
            });
        }
        if prefix.len() < 16 {
            return Ok(None);
        }
        let id = i32::from_be_bytes([prefix[12], prefix[13], prefix[14], prefix[15]]);
        let heartbeat = matches!(
            message_type,
            RequestType::HeartbeatRequest | RequestType::HeartbeatResponse
        );
        if heartbeat && full != head {
            return Err(ProtocolError::InvalidFrame {
                reason: "heartbeat must have no body",
            });
        }
        if !heartbeat && full == head {
            return Err(ProtocolError::InvalidFrame {
                reason: "ordinary frame requires a body",
            });
        }
        Ok(Some((
            full,
            head,
            RpcMessage {
                id,
                message_type,
                codec,
                compressor,
                head_map: Vec::new(),
                body: None,
            },
        )))
    }

    /// 参数必须为一条完整原生V1帧；返回字段对象，截断/尾随/畸形/超限/未实现能力均拒绝。
    pub fn decode(&self, input: &[u8]) -> Result<RpcMessage, ProtocolError> {
        let mut reader = WireReader::new(input, self.limits.maximum_frame_length)?;
        let (full, head, mut rpc) = self.header(input)?.ok_or(ProtocolError::Truncated {
            field: "fixed header",
            needed: 16,
            remaining: input.len(),
        })?;
        if full < input.len() {
            return Err(ProtocolError::TrailingBytes {
                remaining: input.len() - full,
            });
        }
        if full > input.len() {
            return Err(ProtocolError::Truncated {
                field: "frame",
                needed: full,
                remaining: input.len(),
            });
        }
        reader.take(16, "fixed header")?;
        let head_map = HeadMapSerializer::decode(
            reader.take(head - 16, "head map")?,
            self.limits.maximum_header_entries,
        )?;
        let body_bytes = reader.take(full - head, "frame body")?;
        let body = if matches!(
            rpc.message_type,
            RequestType::HeartbeatRequest | RequestType::HeartbeatResponse
        ) {
            if !body_bytes.is_empty() {
                return Err(ProtocolError::InvalidFrame {
                    reason: "heartbeat must have no body",
                });
            }
            None
        } else {
            if body_bytes.is_empty() {
                return Err(ProtocolError::InvalidFrame {
                    reason: "ordinary frame requires a body",
                });
            }
            Some(PayloadCodec::with_limits(self.limits.payload_limits).decode(body_bytes)?)
        };
        reader.finish()?;
        rpc.head_map = head_map;
        rpc.body = body;
        Ok(rpc)
    }
}
