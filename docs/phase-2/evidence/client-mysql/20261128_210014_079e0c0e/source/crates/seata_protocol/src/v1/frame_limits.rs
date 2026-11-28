use crate::{PayloadLimits, ProtocolError};

/// 帧/头条目及载荷树的显式预算；最大帧不超过Java signed-int，最大头不超过signed-short。
/// 对应 Java: ProtocolConstants / ProtocolDecoderV1 的资源边界；新增Rust可配置策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLimits {
    pub(crate) maximum_frame_length: usize,
    pub(crate) maximum_head_length: usize,
    pub(crate) maximum_header_entries: usize,
    pub(crate) payload_limits: PayloadLimits,
}
impl FrameLimits {
    /// 参数依次为整帧字节、含固定16字节的头长度、头条目数、已校验载荷预算。
    /// 返回预算；要求16<=头预算<=帧预算，头最多32767，帧最多2147483647。
    pub fn new(
        maximum_frame_length: usize,
        maximum_head_length: usize,
        maximum_header_entries: usize,
        payload_limits: PayloadLimits,
    ) -> Result<Self, ProtocolError> {
        if maximum_frame_length < 16
            || maximum_frame_length > i32::MAX as usize
            || maximum_head_length < 16
            || maximum_head_length > i16::MAX as usize
            || maximum_head_length > maximum_frame_length
        {
            return Err(ProtocolError::InvalidConfiguration {
                reason: "16 <= head <= frame, head <= 32767, frame <= 2147483647 required",
            });
        }
        Ok(Self {
            maximum_frame_length,
            maximum_head_length,
            maximum_header_entries,
            payload_limits,
        })
    }
    /// 返回整帧字节预算，含固定头、HeadMap和body。
    pub const fn maximum_frame_length(&self) -> usize {
        self.maximum_frame_length
    }
    /// 返回包含固定16字节的头预算。
    pub const fn maximum_head_length(&self) -> usize {
        self.maximum_head_length
    }
    /// 返回头条目数量预算；0仅允许空头。
    pub const fn maximum_header_entries(&self) -> usize {
        self.maximum_header_entries
    }
    /// 返回独立载荷树预算；实际载荷还须符合剩余整帧空间。
    pub const fn payload_limits(&self) -> PayloadLimits {
        self.payload_limits
    }
}
