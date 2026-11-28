use crate::MetadataError;

/// Rust扩展：读取固定JDK导出的兼容性输入，拒绝截断或超出声明格式的数据。
/// 对应 Java: MetadataLocaleOracle生成格式；不是新增原sqlparser对象。
pub(crate) struct CompatibilityDataReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> CompatibilityDataReader<'a> {
    /// 用bytes建立读取游标，返回独立读取器；不修改输入。
    /// 对应 Java: 兼容性输入的DataInput读取边界。
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], MetadataError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(MetadataError::InvalidCompatibilityData("length overflow"))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(MetadataError::InvalidCompatibilityData("truncated input"))?;
        self.position = end;
        Ok(value)
    }
    /// 无参数，返回尚未读取的字节数。
    /// 对应 Java: DataInput兼容性记录边界。
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
    /// 无参数，读取一个无符号字节，截断时返回错误。
    /// 对应 Java: DataInput#readUnsignedByte
    pub(crate) fn byte(&mut self) -> Result<u8, MetadataError> {
        Ok(self.take(1)?[0])
    }
    /// 无参数，读取大端u16，截断时返回错误。
    /// 对应 Java: DataInput#readUnsignedShort
    pub(crate) fn u16(&mut self) -> Result<u16, MetadataError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    /// 无参数，读取大端u32，截断时返回错误。
    /// 对应 Java: DataInput#readInt的无符号位表示。
    pub(crate) fn u32(&mut self) -> Result<u32, MetadataError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// 无参数，读取u32长度前缀的原始字节，截断时返回错误。
    /// 对应 Java: DataInput#readFully
    pub(crate) fn bytes(&mut self) -> Result<Vec<u8>, MetadataError> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    /// 无参数，读取u32长度前缀的大端short位数组，截断时返回错误。
    /// 对应 Java: DataInput#readShort循环。
    pub(crate) fn shorts(&mut self) -> Result<Vec<u16>, MetadataError> {
        let n = self.u32()? as usize;
        let length = n
            .checked_mul(2)
            .ok_or(MetadataError::InvalidCompatibilityData("array overflow"))?;
        Ok(self
            .take(length)?
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect())
    }
    /// 无参数，读取u32长度前缀的大端int位数组，截断时返回错误。
    /// 对应 Java: DataInput#readInt循环。
    pub(crate) fn ints(&mut self) -> Result<Vec<u32>, MetadataError> {
        let n = self.u32()? as usize;
        let length = n
            .checked_mul(4)
            .ok_or(MetadataError::InvalidCompatibilityData("array overflow"))?;
        Ok(self
            .take(length)?
            .chunks_exact(4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .collect())
    }
    /// 无参数，读取u16长度前缀的UTF16码元；孤立代理码元不被替换。
    /// 对应 Java: DataInput#readChar循环。
    pub(crate) fn string_units(&mut self) -> Result<Vec<u16>, MetadataError> {
        let n = usize::from(self.u16()?);
        (0..n).map(|_| self.u16()).collect()
    }
    /// 无参数，检查全部输入已经消费，剩余字节返回格式错误。
    /// 对应 Java: 固定兼容性文件的完整消费门禁。
    pub(crate) fn finish(&self) -> Result<(), MetadataError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(MetadataError::InvalidCompatibilityData("trailing bytes"))
        }
    }
}
