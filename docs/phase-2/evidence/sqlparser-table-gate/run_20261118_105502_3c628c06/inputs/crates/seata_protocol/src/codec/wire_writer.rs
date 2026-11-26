use crate::ProtocolError;
use std::fmt;

/// 有明确预算的大端字段写入器；字段出错不追加部分字段，Debug不泄露载荷。
/// 对应 Java: io.netty.buffer.ByteBuf 及各 MessageSeataCodec#encode 的字段写入职责。
pub struct WireWriter {
    bytes: Vec<u8>,
    maximum: usize,
}

impl fmt::Debug for WireWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WireWriter")
            .field("length", &self.bytes.len())
            .field("maximum", &self.maximum)
            .finish()
    }
}

impl WireWriter {
    /// 参数maximum为显式字节预算；返回空写入器，不预分配maximum大小的内存。
    pub const fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    fn ensure(&self, additional: usize) -> Result<(), ProtocolError> {
        let actual =
            self.bytes
                .len()
                .checked_add(additional)
                .ok_or(ProtocolError::LimitExceeded {
                    limit: "output bytes",
                    maximum: self.maximum,
                    actual: usize::MAX,
                })?;
        if actual > self.maximum {
            return Err(ProtocolError::LimitExceeded {
                limit: "output bytes",
                maximum: self.maximum,
                actual,
            });
        }
        Ok(())
    }

    /// 参数为原始字节；预算足够时完整追加，否则保持原输出不变。
    pub fn write_bytes(&mut self, value: &[u8]) -> Result<(), ProtocolError> {
        self.ensure(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    /// 写入单字节；超出预算时返回错误。
    pub fn u8(&mut self, value: u8) -> Result<(), ProtocolError> {
        self.write_bytes(&[value])
    }
    /// 写入大端无符号short；超出预算时返回错误。
    pub fn u16(&mut self, value: u16) -> Result<(), ProtocolError> {
        self.write_bytes(&value.to_be_bytes())
    }
    /// 写入Java有符号short，保留符号位；超出预算时返回错误。
    pub fn i16(&mut self, value: i16) -> Result<(), ProtocolError> {
        self.write_bytes(&value.to_be_bytes())
    }
    /// 写入大端无符号int，供帧字段使用；超出预算时返回错误。
    pub fn u32(&mut self, value: u32) -> Result<(), ProtocolError> {
        self.write_bytes(&value.to_be_bytes())
    }
    /// 写入Java有符号int；超出预算时返回错误。
    pub fn i32(&mut self, value: i32) -> Result<(), ProtocolError> {
        self.write_bytes(&value.to_be_bytes())
    }

    /// 在已写入的四字节占位处回填长度；内部使用，越界返回错误且不修改已有输出。
    pub(crate) fn replace_i32(&mut self, offset: usize, value: i32) -> Result<(), ProtocolError> {
        let end = offset.checked_add(4).ok_or(ProtocolError::InvalidValue {
            field: "length_offset",
            reason: "offset overflow",
        })?;
        let target = self
            .bytes
            .get_mut(offset..end)
            .ok_or(ProtocolError::InvalidValue {
                field: "length_offset",
                reason: "placeholder must already exist",
            })?;
        target.copy_from_slice(&value.to_be_bytes());
        Ok(())
    }
    /// 写入Java有符号long；超出预算时返回错误。
    pub fn i64(&mut self, value: i64) -> Result<(), ProtocolError> {
        self.write_bytes(&value.to_be_bytes())
    }

    fn short_text(
        &mut self,
        value: Option<&str>,
        field: &'static str,
        nullable: bool,
    ) -> Result<(), ProtocolError> {
        let text = value.unwrap_or("");
        let length = i16::try_from(text.len()).map_err(|_| ProtocolError::InvalidLength {
            field,
            length: i64::try_from(text.len()).unwrap_or(i64::MAX),
        })?;
        self.ensure(2 + text.len())?;
        let prefix = if nullable && value.is_none() {
            -1i16
        } else {
            length
        };
        self.bytes.extend_from_slice(&prefix.to_be_bytes());
        self.bytes.extend_from_slice(text.as_bytes());
        Ok(())
    }

    /// 写入普通string16；None和空串均编码为0，UTF-8字节长度最多32767，不截断文本。
    /// 参数field用于错误定位；超长/超预算时保持原输出不变。
    pub fn string16(
        &mut self,
        value: Option<&str>,
        field: &'static str,
    ) -> Result<(), ProtocolError> {
        self.short_text(value, field, false)
    }

    /// 写入HeadMap nullable string16；None编码-1，空串编码0，参数field用于错误定位。
    pub fn nullable_string16(
        &mut self,
        value: Option<&str>,
        field: &'static str,
    ) -> Result<(), ProtocolError> {
        self.short_text(value, field, true)
    }

    /// 写入signed-int长度的UTF-8字段；None/空串编码0，长度或预算错误不追加部分字段。
    pub fn string32(
        &mut self,
        value: Option<&str>,
        field: &'static str,
    ) -> Result<(), ProtocolError> {
        let text = value.unwrap_or("");
        let length = i32::try_from(text.len()).map_err(|_| ProtocolError::InvalidLength {
            field,
            length: i64::try_from(text.len()).unwrap_or(i64::MAX),
        })?;
        self.ensure(4 + text.len())?;
        self.bytes.extend_from_slice(&length.to_be_bytes());
        self.bytes.extend_from_slice(text.as_bytes());
        Ok(())
    }

    /// 将参数bool写入规范的双字节0/1；预算不足时返回错误。
    pub fn boolean16(&mut self, value: bool) -> Result<(), ProtocolError> {
        self.u16(u16::from(value))
    }
    /// 返回当前已写入的字节数。
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    /// 返回当前是否还没有写入字段。
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    /// 返回当前编码字节的只读借用，不能绕过预算修改缓冲区。
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// 消耗写入器并返回已编码字节；不添加额外帧或类型前缀。
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
