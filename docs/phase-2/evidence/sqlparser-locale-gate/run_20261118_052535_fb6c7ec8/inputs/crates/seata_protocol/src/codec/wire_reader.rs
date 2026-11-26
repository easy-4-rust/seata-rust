use crate::ProtocolError;
use std::fmt;

/// 严格大端字段游标；先检查长度再读取/分配，Debug不展示载荷内容。
/// 对应 Java: java.nio.ByteBuffer 及各 MessageSeataCodec#decode 的字段读取职责。
/// 复合字段报错时长度前缀或字段内容可能已消费；调用方应丢弃此游标，不吞错继续解码。
/// 单次take的截断错误不会推进位置，但不提供整个消息的错误恢复机制。
#[derive(Clone)]
pub struct WireReader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl fmt::Debug for WireReader<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WireReader")
            .field("length", &self.input.len())
            .field("offset", &self.offset)
            .finish()
    }
}

impl<'a> WireReader<'a> {
    /// 参数为待解析字节和最大允许长度；超出预算返回错误，否则返回借用游标。
    pub fn new(input: &'a [u8], maximum: usize) -> Result<Self, ProtocolError> {
        if input.len() > maximum {
            return Err(ProtocolError::LimitExceeded {
                limit: "input bytes",
                maximum,
                actual: input.len(),
            });
        }
        Ok(Self { input, offset: 0 })
    }

    /// 返回尚未消费的字节数，不改变游标。
    pub const fn remaining(&self) -> usize {
        self.input.len() - self.offset
    }

    /// 参数为长度及字段名；返回有界切片并推进游标，截断时位置保持不变。
    pub fn take(&mut self, length: usize, field: &'static str) -> Result<&'a [u8], ProtocolError> {
        let remaining = self.remaining();
        if length > remaining {
            return Err(ProtocolError::Truncated {
                field,
                needed: length,
                remaining,
            });
        }
        // length <= remaining，故offset + length不溢出且不越界。
        let start = self.offset;
        self.offset += length;
        Ok(&self.input[start..self.offset])
    }

    /// 返回下一个大端短整数而不消费；缺少双字节类型前缀时返回截断错误。
    pub fn peek_u16(&self) -> Result<u16, ProtocolError> {
        if self.remaining() < 2 {
            return Err(ProtocolError::Truncated {
                field: "u16",
                needed: 2,
                remaining: self.remaining(),
            });
        }
        Ok(u16::from_be_bytes([
            self.input[self.offset],
            self.input[self.offset + 1],
        ]))
    }

    /// 读取并返回单字节；输入不足时返回截断错误。
    pub fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1, "u8")?[0])
    }
    /// 读取并返回大端无符号短整数；输入不足时返回错误。
    pub fn u16(&mut self) -> Result<u16, ProtocolError> {
        let bytes = self.take(2, "u16")?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }
    /// 读取Java有符号short，不进行业务值校验；输入不足时返回错误。
    pub fn i16(&mut self) -> Result<i16, ProtocolError> {
        let bytes = self.take(2, "i16")?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }
    /// 读取并返回大端无符号四字节整数，供帧长度/ID字段使用。
    pub fn u32(&mut self) -> Result<u32, ProtocolError> {
        let bytes = self.take(4, "u32")?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    /// 读取Java有符号int；输入不足时返回错误，不丢弃符号。
    pub fn i32(&mut self) -> Result<i32, ProtocolError> {
        let bytes = self.take(4, "i32")?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    /// 读取Java有符号long；输入不足时返回错误，不丢弃符号。
    pub fn i64(&mut self) -> Result<i64, ProtocolError> {
        let bytes = self.take(8, "i64")?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn text(&mut self, length: usize, field: &'static str) -> Result<String, ProtocolError> {
        let bytes = self.take(length, field)?;
        let text = std::str::from_utf8(bytes).map_err(|_| ProtocolError::InvalidUtf8 { field })?;
        Ok(text.to_owned())
    }

    /// 读取signed-short长度的UTF-8；0返回None，负值/损坏UTF-8/截断返回错误。
    /// 参数field用于定位错误；调用消息模型须自行落实Java各字段的空值语义。
    pub fn string16(&mut self, field: &'static str) -> Result<Option<String>, ProtocolError> {
        let length = self.i16()?;
        if length < 0 {
            return Err(ProtocolError::InvalidLength {
                field,
                length: i64::from(length),
            });
        }
        if length == 0 {
            return Ok(None);
        }
        Ok(Some(self.text(length as usize, field)?))
    }

    /// 读取signed-int长度的UTF-8；0返回None，负值/截断/非法UTF-8拒绝后不分配文本。
    pub fn string32(&mut self, field: &'static str) -> Result<Option<String>, ProtocolError> {
        let length = self.i32()?;
        if length < 0 {
            return Err(ProtocolError::InvalidLength {
                field,
                length: i64::from(length),
            });
        }
        if length == 0 {
            return Ok(None);
        }
        Ok(Some(self.text(length as usize, field)?))
    }

    /// 读取HeadMap的nullable string16；-1返回None，0返回Some空串，其他负值拒绝。
    pub fn nullable_string16(
        &mut self,
        field: &'static str,
    ) -> Result<Option<String>, ProtocolError> {
        let length = self.i16()?;
        match length {
            -1 => Ok(None),
            0..=i16::MAX => Ok(Some(self.text(length as usize, field)?)),
            _ => Err(ProtocolError::InvalidLength {
                field,
                length: i64::from(length),
            }),
        }
    }

    /// 读取双字节布尔字段；只接受0/1，其他值返回带字段名的错误。
    pub fn boolean16(&mut self, field: &'static str) -> Result<bool, ProtocolError> {
        match self.u16()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(ProtocolError::InvalidBoolean { field, value }),
        }
    }

    /// 校验一个完整对象已消费全部输入；存在尾随字节返回错误，不改变位置。
    pub fn finish(&self) -> Result<(), ProtocolError> {
        let remaining = self.remaining();
        if remaining == 0 {
            Ok(())
        } else {
            Err(ProtocolError::TrailingBytes { remaining })
        }
    }
}
