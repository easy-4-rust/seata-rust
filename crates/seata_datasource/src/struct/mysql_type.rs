use crate::{ColumnDomain, ColumnMeta, MetadataError};

/// 原生MySQL类型身份；不将相同Rust值域误当成相同JDBC类型或包装类。
/// 对应 Java: com.mysql.cj.MysqlType及ColumnMeta#dataTypeName（当前已认证物理类型子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MySqlType {
    /// MySQL DATETIME，保留 0–6 位小数秒的独立物理域。
    DateTime,
    /// 原生 JSON；JDBC 使用 LONGVARCHAR/String，仍独立于 TEXT 保存物理身份。
    Json,
    /// TINYINT，保留驱动boolean配置会使用的显示宽度1信息；unsigned由列域独立保存。
    TinyInt {
        /// 声明是否保留TINYINT(1)显示宽度；不自动把物理整数转换为bool。
        display_width_one: bool,
    },
    /// SMALLINT。
    SmallInt,
    /// MEDIUMINT。
    MediumInt,
    /// INT。
    Int,
    /// BIGINT。
    BigInt,
    /// 精确DECIMAL。
    Decimal,
    /// CHAR。
    Char,
    /// VARCHAR。
    VarChar,
    /// TINYTEXT，原驱动与其他TEXT的JDBC类型可能不同。
    TinyText,
    /// TEXT。
    Text,
    /// MEDIUMTEXT。
    MediumText,
    /// LONGTEXT。
    LongText,
    /// BINARY。
    Binary,
    /// VARBINARY。
    VarBinary,
    /// TINYBLOB，原驱动与其他BLOB的JDBC类型可能不同。
    TinyBlob,
    /// BLOB。
    Blob,
    /// MEDIUMBLOB。
    MediumBlob,
    /// LONGBLOB。
    LongBlob,
}
impl MySqlType {
    /// 参数为完整列域；返回原生类型和物理域的一致性，不推断缺失域。
    pub(crate) fn validate(self, column: &ColumnMeta) -> Result<(), MetadataError> {
        let valid = matches!(
            (self, column.domain()),
            (Self::DateTime, Some(ColumnDomain::LocalDateTime { .. }))
                | (Self::Json, Some(ColumnDomain::Json))
                | (
                    Self::TinyInt { .. },
                    Some(ColumnDomain::Integer { bits: 8, .. })
                )
                | (Self::SmallInt, Some(ColumnDomain::Integer { bits: 16, .. }))
                | (
                    Self::MediumInt,
                    Some(ColumnDomain::Integer { bits: 24, .. })
                )
                | (Self::Int, Some(ColumnDomain::Integer { bits: 32, .. }))
                | (Self::BigInt, Some(ColumnDomain::Integer { bits: 64, .. }))
                | (Self::Decimal, Some(ColumnDomain::Decimal { .. }))
                | (
                    Self::Char,
                    Some(ColumnDomain::Text {
                        fixed_width: true,
                        character_limit: Some(_),
                        ..
                    }),
                )
                | (
                    Self::VarChar,
                    Some(ColumnDomain::Text {
                        fixed_width: false,
                        character_limit: Some(_),
                        ..
                    }),
                )
                | (
                    Self::TinyText | Self::Text | Self::MediumText | Self::LongText,
                    Some(ColumnDomain::Text {
                        fixed_width: false,
                        character_limit: None,
                        ..
                    }),
                )
                | (
                    Self::Binary,
                    Some(ColumnDomain::Binary { fixed_width: true })
                )
                | (
                    Self::VarBinary
                        | Self::TinyBlob
                        | Self::Blob
                        | Self::MediumBlob
                        | Self::LongBlob,
                    Some(ColumnDomain::Binary { fixed_width: false }),
                )
        );
        if valid {
            Ok(())
        } else {
            Err(MetadataError::Invalid {
                reason: "native MySQL type and physical domain",
            })
        }
    }
}
