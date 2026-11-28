use crate::{ColumnMeta, ColumnType, FieldValue, ImageError};

/// 不可变字段，列属性和精确值在构造时一起验证；Debug不输出值。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field；仅已认证MySQL值域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    column: ColumnMeta,
    value: FieldValue,
}
impl Field {
    /// 参数为完整列元数据和已拥有的精确值；返回类型、NULL及长度均匹配的字段。
    pub fn new(column: &ColumnMeta, value: FieldValue) -> Result<Self, ImageError> {

        let matches = match (&value, column.kind()) {
            (FieldValue::Null, _) => column.nullable(),
            (FieldValue::Signed(_), ColumnType::SignedInteger)
            | (FieldValue::Unsigned(_), ColumnType::UnsignedInteger) => true,
            (FieldValue::Text(_), ColumnType::Text)
            | (FieldValue::Binary(_), ColumnType::Binary) => {
                // JSON 没有字符维度；全局字段预算仍在上方执行，其他文本保持原长度校验。
                matches!(column.domain(), Some(crate::ColumnDomain::Json))
                    || column
                        .char_octet_length()
                        .is_some_and(|limit| value.value_bytes() as u64 <= limit)
            }
            (FieldValue::Decimal(v), ColumnType::Decimal) => {
                matches!((column.numeric_precision(),column.numeric_scale()),(Some(p),Some(s))
                    if v.scale()==s && v.integer_digits()<=p-s)
            }
            _ => false,
        };
        if !matches
            || column
                .domain()
                .is_some_and(|domain| !domain.accepts(column, &value))
        {
            return Err(ImageError::Invalid {
                reason: "column value domain, nullability or dimensions",
            });
        }
        Ok(Self {
            column: column.clone(),
            value,
        })
    }
    /// 无参数；返回列的原名。
    pub fn name(&self) -> &str {
        self.column.name()
    }
    /// 无参数；返回经过验证的完整列元数据。
    pub fn column(&self) -> &ColumnMeta {
        &self.column
    }
    /// 无参数；返回精确值的只读引用。
    pub fn value(&self) -> &FieldValue {
        &self.value
    }
    /// 参数为列名；忽略ASCII大小写判断是否为同名字段，对应Java Field#isKey。
    pub fn is_key(&self, pk_name: &str) -> bool {
        self.name().eq_ignore_ascii_case(pk_name)
    }
    /// 无参数；返回字段是否属于主键。
    pub const fn is_primary_key(&self) -> bool {
        self.column.primary_key_ordinal().is_some()
    }
    /// 无参数；返回值负载字节数。
    pub fn value_bytes(&self) -> usize {
        self.value.value_bytes()
    }
}
