use crate::{CharacterSet, ColumnMeta, ColumnType, FieldValue, MetadataError};

/// 列的物理值域，补足通用整数/字节类型无法表达的约束；列构造时必须核对一致性。
/// 对应 Java: org.apache.seata.sqlparser.struct.ColumnMeta的类型、长度、精度与unsigned；Rust结构化扩展。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnDomain {
    /// MySQL五种整型宽度，位数仅8/16/24/32/64。
    Integer {
        /// 实际存储位数，不是显示宽度。
        bits: u8,
        /// 是否无符号。
        unsigned: bool,
    },
    /// 文本声明；CHAR/VARCHAR有字符上限，TEXT只使用列octet上限。
    Text {
        /// 目标字符集。
        charset: CharacterSet,
        /// CHAR/VARCHAR字符数，TEXT为None。
        character_limit: Option<u64>,
        /// 是否CHAR定长逻辑值；读取仍遵循服务端去尾空格语义。
        fixed_width: bool,
    },
    /// 二进制声明，定长BINARY读取必须包含补齐的零字节。
    Binary {
        /// 是否BINARY定长，VARBINARY/BLOB为false。
        fixed_width: bool,
    },
    /// 精确十进制符号约束，精度与scale仍由列元数据保存。
    Decimal {
        /// 是否禁止负数。
        unsigned: bool,
    },
}
impl ColumnDomain {
    /// 参数为完整列元数据；返回物理域与通用值域/维度的一致性结果。
    pub(crate) fn validate(&self, column: &ColumnMeta) -> Result<(), MetadataError> {
        let valid = match self {
            Self::Integer { bits, unsigned } => {
                matches!(bits, 8 | 16 | 24 | 32 | 64)
                    && column.kind()
                        == if *unsigned {
                            ColumnType::UnsignedInteger
                        } else {
                            ColumnType::SignedInteger
                        }
            }
            Self::Text {
                charset,
                character_limit,
                fixed_width,
            } => {
                column.kind() == ColumnType::Text
                    && column.char_octet_length().is_some()
                    && match character_limit {
                        Some(limit) => {
                            *limit > 0
                                && limit.checked_mul(charset.maximum_octets())
                                    == column.char_octet_length()
                        }
                        None => !fixed_width,
                    }
            }
            Self::Binary { .. } => {
                column.kind() == ColumnType::Binary && column.char_octet_length().is_some()
            }
            Self::Decimal { .. } => column.kind() == ColumnType::Decimal,
        };
        if valid {
            Ok(())
        } else {
            Err(MetadataError::Invalid {
                reason: "physical column domain mismatch",
            })
        }
    }
    /// 参数为已验证列元数据及通用类型正确的字段值；返回物理范围和编码是否兼容。
    pub(crate) fn accepts(&self, column: &ColumnMeta, value: &FieldValue) -> bool {
        if matches!(value, FieldValue::Null) {
            return true;
        }
        match (self, value) {
            (
                Self::Integer {
                    bits,
                    unsigned: false,
                },
                FieldValue::Signed(v),
            ) => {
                let bound = 1i128 << (bits - 1);
                i128::from(*v) >= -bound && i128::from(*v) < bound
            }
            (
                Self::Integer {
                    bits,
                    unsigned: true,
                },
                FieldValue::Unsigned(v),
            ) => u128::from(*v) < (1u128 << bits),
            (
                Self::Text {
                    charset,
                    character_limit,
                    ..
                },
                FieldValue::Text(v),
            ) => {
                charset.accepts(v)
                    && character_limit.is_none_or(|limit| v.chars().count() as u64 <= limit)
            }
            (Self::Binary { fixed_width }, FieldValue::Binary(v)) => {
                !fixed_width || Some(v.len() as u64) == column.char_octet_length()
            }
            (Self::Decimal { unsigned }, FieldValue::Decimal(v)) => {
                true
            }
            _ => false,
        }
    }
}
