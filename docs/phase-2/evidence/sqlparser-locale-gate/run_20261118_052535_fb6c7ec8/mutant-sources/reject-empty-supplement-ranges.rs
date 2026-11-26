use crate::{compatibility_data_reader::CompatibilityDataReader, MetadataError};

/// Rust扩展：固定JDK分词/字典使用的压缩字符分类数据。
/// 对应 Java: sun.text.CompactByteArray和SupplementaryCharacterData的只读查询，不计原对象。
pub(crate) struct JdkCharacterCategories {
    indices: Vec<u16>,
    values: Vec<u8>,
    supplementary: Vec<u32>,
}

impl JdkCharacterCategories {
    /// 从reader读取一组分类表，返回经边界检查的压缩表；格式无效返回Rust错误。
    /// 对应 Java: 原分类表的导出数据格式。
    pub(crate) fn read(reader: &mut CompatibilityDataReader<'_>) -> Result<Self, MetadataError> {
        let value = Self {
            indices: reader.shorts()?,
            values: reader.bytes()?,
            supplementary: reader.ints()?,
        };
        if value.indices.len() != 512
            || value
                .indices
                .iter()
                .any(|i| usize::from(*i) + 127 >= value.values.len())
            || value.supplementary.len() < 2
            || value.supplementary[0] >> 8 != 0x10000
            || value.supplementary.last().copied().map(|v| v >> 8) != Some(0x110000)
            || value
                .supplementary
                .windows(2)
                .any(|w| w[0] >> 8 >= w[1] >> 8)
        {
            return Err(MetadataError::InvalidCompatibilityData(
                "character categories",
            ));
        }
        Ok(value)
    }
    /// 查询有效code_point分类，返回有符号分类；-1保留原IGNORE语义，无状态修改。
    /// 对应 Java: CompactByteArray#elementAt和SupplementaryCharacterData#getValue
    pub(crate) fn category(&self, code_point: u32) -> i32 {
        let value = if code_point < 0x10000 {
            let i = code_point as usize;
            self.values[usize::from(self.indices[i >> 7]) + (i & 127)]
        } else {
            let index = self
                .supplementary
                .partition_point(|v| *v >> 8 <= code_point);
            (self.supplementary[index.saturating_sub(1)] & 0xff) as u8
        };
        i32::from(value as i8)
    }
}
