use crate::ProtocolError;

/// CompressorType协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.compressor.CompressorType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressorType {
    /// 无压缩，首期实现。
    None,
    /// Gzip。
    Gzip,
    /// Zip。
    Zip,
    /// 7z。
    Sevenz,
    /// Bzip2。
    Bzip2,
    /// Lz4。
    Lz4,
    /// Deflate。
    Deflater,
    /// Zstandard。
    Zstd,
}

impl CompressorType {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::None),
            1 => Ok(Self::Gzip),
            2 => Ok(Self::Zip),
            3 => Ok(Self::Sevenz),
            4 => Ok(Self::Bzip2),
            5 => Ok(Self::Lz4),
            6 => Ok(Self::Deflater),
            7 => Ok(Self::Zstd),
            _ => Err(ProtocolError::UnknownCode {
                kind: "CompressorType",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Gzip => 1,
            Self::Zip => 2,
            Self::Sevenz => 3,
            Self::Bzip2 => 4,
            Self::Lz4 => 5,
            Self::Deflater => 6,
            Self::Zstd => 7,
        }
    }
}
