/// 明确选择已现场对照的Connector8字段配置；不代表所有JDBC驱动版本。
/// 对应 Java: com.mysql.cj.jdbc.DatabaseMetaData的tinyInt1isBit配置；Rust显式转换策略扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MySqlJdbcProfile {
    /// 原 Connector8 Boolean 映射，额外要求 Java Jackson LocalDateTime 序列化为 UTC；持久码 3。
    Connector8DefaultUtc,
    /// 原 Connector8 数字 TINYINT 映射，额外要求 Java Jackson LocalDateTime 序列化为 UTC；持久码 4。
    Connector8NumericTinyIntUtc,
    /// 原8.0.27默认配置；signed TINYINT(1)只对物理0/1允许无损Boolean编码。
    Connector8Default,
    /// 原8.0.27 tinyInt1isBit=false，完整保留整数值。
    Connector8NumericTinyInt,
}

impl MySqlJdbcProfile {
    /// 无参数；返回稳定持久编号，保留原 1/2 编号，不按 Rust 枚举顺序转换。
    pub const fn storage_code(self) -> u8 {
        match self {
            Self::Connector8Default => 1,
            Self::Connector8NumericTinyInt => 2,
            Self::Connector8DefaultUtc => 3,
            Self::Connector8NumericTinyIntUtc => 4,
        }
    }
    /// 参数为持久编号；返回原明确配置，未知编号不猜测。
    pub const fn from_storage_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Connector8Default),
            2 => Some(Self::Connector8NumericTinyInt),
            3 => Some(Self::Connector8DefaultUtc),
            4 => Some(Self::Connector8NumericTinyIntUtc),
            _ => None,
        }
    }
    /// 无参数；返回该配置是否使用原 Boolean TINYINT(1) 映射。
    pub const fn boolean_tinyint(self) -> bool {
        matches!(self, Self::Connector8Default | Self::Connector8DefaultUtc)
    }
    /// 无参数；返回是否明确约定 Java LocalDateTime 的 UTC 序列化。
    pub const fn utc_temporal(self) -> bool {
        matches!(
            self,
            Self::Connector8DefaultUtc | Self::Connector8NumericTinyIntUtc
        )
    }
}
