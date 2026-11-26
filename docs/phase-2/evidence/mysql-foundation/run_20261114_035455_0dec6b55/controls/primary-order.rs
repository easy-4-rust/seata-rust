use crate::{ColumnMeta, MetadataError};
use std::collections::HashSet;

/// 表元数据，物理列序与主键索引序独立保存；不跨事务承诺schema仍然有效。
/// 对应 Java: org.apache.seata.sqlparser.struct.TableMeta。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMeta {
    name: String,
    columns: Vec<ColumnMeta>,
    primary_indices: Vec<usize>,
}
impl TableMeta {
    /// 参数为原表名及按物理顺序排列的完整列；返回已检查非空主键的表元数据。
    pub fn new(name: String, columns: Vec<ColumnMeta>) -> Result<Self, MetadataError> {
        Self::validate_identifier(&name)?;
        if columns.is_empty() || columns.len() > 128 {
            return Err(MetadataError::Budget {
                limit: "1..128 columns",
            });
        }
        let mut names = HashSet::new();
        let mut primary = Vec::new();
        for (index, column) in columns.iter().enumerate() {
            if column.ordinal() as usize != index + 1
                || !names.insert(column.name().to_ascii_lowercase())
            {
                return Err(MetadataError::Invalid {
                    reason: "physical column order or duplicate name",
                });
            }
            if let Some(position) = column.primary_key_ordinal() {
                primary.push((position, index));
            }
        }
        primary.sort_unstable();
        if primary.is_empty()
            || primary
                .iter()
                .enumerate()
                .any(|(index, (position, _))| *position as usize != index + 1)
        {
            return Err(MetadataError::Invalid {
                reason: "complete unique primary key order required",
            });
        }
        Ok(Self {
            name,
            columns,
            primary_indices: primary.into_iter().map(|(position, _)| position as usize - 1).collect(),
        })
    }
    /// 参数为待用标识符；返回长度/字符规则校验结果，不接受引用或schema分隔符。
    pub fn validate_identifier(value: &str) -> Result<(), MetadataError> {
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 64
            || bytes[0].is_ascii_digit()
            || !bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$')
        {
            return Err(MetadataError::Invalid {
                reason: "bounded ASCII identifier required",
            });
        }
        Ok(())
    }
    /// 参数为已声明规则下的标识符；返回安全引用名，验证失败不拼接原字符串。
    pub fn quote_identifier(value: &str) -> Result<String, MetadataError> {
        Self::validate_identifier(value)?;
        Ok(format!("`{value}`"))
    }
    /// 无参数；返回数据库原表名。
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 无参数；返回完整物理列顺序。
    pub fn columns(&self) -> &[ColumnMeta] {
        &self.columns
    }
    /// 无参数；按真实主键索引顺序迭代列，不能用物理列位置代替。
    pub fn primary_key_columns(&self) -> impl Iterator<Item = &ColumnMeta> {
        self.primary_indices
            .iter()
            .map(|index| &self.columns[*index])
    }
    /// 参数为列名；按MySQL列名大小写规则返回匹配列，缺失返回None。
    pub fn column(&self, name: &str) -> Option<&ColumnMeta> {
        self.columns
            .iter()
            .find(|column| column.name().eq_ignore_ascii_case(name))
    }
}
