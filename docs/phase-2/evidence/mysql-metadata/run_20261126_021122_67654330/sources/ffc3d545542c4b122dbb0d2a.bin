/// 当前已验证文本字符集，不把UTF-8可解码自动视为目标列可表示。
/// 对应 Java: org.apache.seata.sqlparser.struct.ColumnMeta的字符集元数据；Rust有限枚举扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterSet {
    /// 单字节ASCII。
    Ascii,
    /// 最多三字节UTF-8，不接受补充平面字符。
    Utf8Mb3,
    /// 完整UTF-8，不进行Unicode规范化。
    Utf8Mb4,
}
impl CharacterSet {
    /// 无参数；返回每字符最大编码字节，用于核对CHAR/VARCHAR声明维度。
    pub const fn maximum_octets(self) -> u64 {
        match self {
            Self::Ascii => 1,
            Self::Utf8Mb3 => 3,
            Self::Utf8Mb4 => 4,
        }
    }
    /// 参数为UTF-8文本；返回该字符集能否精确表示全部字符。
    pub fn accepts(self, text: &str) -> bool {
        match self {
            Self::Ascii => text.is_ascii(),
            Self::Utf8Mb3 => text.chars().all(|c| u32::from(c) <= 0xffff),
            Self::Utf8Mb4 => true,
        }
    }
}
