use crate::TransportError;
use seata_protocol::protocol::RpcMessage;
use seata_protocol::v1::{FrameLimits, ProtocolDecoderV1};
use std::fmt;

/// 每连接独占的增量帧解码器；一次返回一帧，只缓存该帧尚未完整的字节。
/// 对应 Java: org.apache.seata.core.rpc.netty.v1.ProtocolDecoderV1 的LengthFieldBasedFrameDecoder流职责。
/// 不实现Clone，EOF/协议错误后必须由新连接创建新实例。
pub struct FrameDecoder {
    protocol: ProtocolDecoderV1,
    buffer: Vec<u8>,
    expected_frame_length: Option<usize>,
    closed: bool,
}
impl fmt::Debug for FrameDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrameDecoder")
            .field("buffered_bytes", &self.buffer.len())
            .field("reserved_bytes", &self.buffer.capacity())
            .field("expected_frame_length", &self.expected_frame_length)
            .field("closed", &self.closed)
            .finish()
    }
}
impl FrameDecoder {
    /// 参数为已验证帧/头/载荷预算；返回空流状态，不预分配最大帧内存。
    pub const fn new(limits: FrameLimits) -> Self {
        Self {
            protocol: ProtocolDecoderV1::new(limits),
            buffer: Vec::new(),
            expected_frame_length: None,
            closed: false,
        }
    }
    /// 返回该流实例使用的完整协议预算。
    pub const fn limits(&self) -> FrameLimits {
        self.protocol.limits()
    }
    /// 返回当前缓存的残片字节数；不暴露其内容。
    pub fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }
    /// 返回当前缓冲区保留容量，供内存预算监控；不等于已收到的字节数，不暴露内容。
    pub fn reserved_bytes(&self) -> usize {
        self.buffer.capacity()
    }
    /// 返回是否因EOF或错误而永久终止。
    pub const fn is_closed(&self) -> bool {
        self.closed
    }
    /// 参数input为当前网络片段的未消费切片，成功时推进；返回最多一个完整对象。
    /// None表示需要更多输入；完整帧后的后缀留给调用方再次调用。错误永久终止本实例。
    pub fn decode(&mut self, input: &mut &[u8]) -> Result<Option<RpcMessage>, TransportError> {
        if self.closed {
            return Err(TransportError::DecoderClosed);
        }
        if input.is_empty() {
            return Ok(None);
        }
        if self.expected_frame_length.is_none() {
            if let Err(error) = self.fill(input, 16) {
                self.close();
                return Err(error);
            }
            match self.protocol.frame_length(&self.buffer) {
                Ok(Some(length)) => self.expected_frame_length = Some(length),
                Ok(None) => return Ok(None),
                Err(error) => {
                    self.close();
                    return Err(error.into());
                }
            }
        }
        // 固定头已经过协议组件验证，之后才允许按目标长度分配正文空间。
        if let Some(length) = self.expected_frame_length {
            if self.buffer.len() < length && !input.is_empty() {
                if let Err(error) = self.fill(input, length) {
                    self.close();
                    return Err(error);
                }
            }
            if self.buffer.len() < length {
                return Ok(None);
            }
        }
        let result = self.protocol.decode(&self.buffer);
        self.buffer.clear();
        self.expected_frame_length = None;
        match result {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                self.close();
                Err(error.into())
            }
        }
    }
    /// 通知流EOF，返回是否正常结束；残片返回UnexpectedEof，正常或异常均永久终止。
    pub fn finish(&mut self) -> Result<(), TransportError> {
        if self.closed {
            return Err(TransportError::DecoderClosed);
        }
        let buffered = self.buffer.len();
        let expected_frame_length = self.expected_frame_length;
        self.close();
        if buffered == 0 {
            Ok(())
        } else {
            Err(TransportError::UnexpectedEof {
                buffered,
                expected_frame_length,
            })
        }
    }
    fn fill(&mut self, input: &mut &[u8], target: usize) -> Result<(), TransportError> {
        let missing = target - self.buffer.len();
        self.buffer
            .try_reserve_exact(missing)
            .map_err(|_| TransportError::BufferAllocationFailed { requested: target })?;
        let amount = missing.min(input.len());
        self.buffer.extend_from_slice(&input[..amount]);
        *input = &input[amount..];
        Ok(())
    }
    fn close(&mut self) {
        self.closed = true;
        self.buffer.clear();
        self.expected_frame_length = None;
    }
}
