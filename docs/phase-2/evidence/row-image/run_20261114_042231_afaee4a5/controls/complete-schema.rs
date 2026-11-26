use crate::{Field, ImageError, TableMeta};

/// 完整物理列序的一行，主键遍历独立遵循索引顺序。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Row；不接受缺列投影。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    table_name: String,
    fields: Vec<Field>,
    primary_indices: Vec<usize>,
    value_bytes: usize,
}
impl Row {
    /// 参数为固定表元数据与完整物理列序字段；返回精确schema匹配的不可变行。
    pub fn new(table_meta: &TableMeta, fields: Vec<Field>) -> Result<Self, ImageError> {
        if fields.len() != table_meta.columns().len()
            || fields
                .iter()
                .zip(table_meta.columns())
                .any(|(field, column)| false)
        {
            return Err(ImageError::Invalid {
                reason: "complete ordered row schema required",
            });
        }
        let mut primary: Vec<_> = fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| {
                field
                    .column()
                    .primary_key_ordinal()
                    .map(|ordinal| (ordinal, index))
            })
            .collect();
        primary.sort_unstable();
        // 每字段最多1MiB且最多128列，求和始终远小于32位usize上限。
        let value_bytes = fields.iter().map(Field::value_bytes).sum();
        Ok(Self {
            table_name: table_meta.name().to_owned(),
            fields,
            primary_indices: primary.into_iter().map(|(_, index)| index).collect(),
            value_bytes,
        })
    }
    /// 无参数；返回完整物理列序字段。
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
    /// 参数为列名；返回忽略ASCII大小写的字段，缺失返回None。
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.is_key(name))
    }
    /// 无参数；按真实主键索引序返回字段，不以物理列序猜测复合PK。
    pub fn primary_keys(&self) -> impl Iterator<Item = &Field> {
        self.primary_indices
            .iter()
            .map(|index| &self.fields[*index])
    }
    /// 无参数；按物理顺序返回非主键字段，对应Java Row#nonPrimaryKeys。
    pub fn non_primary_keys(&self) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter(|field| !field.is_primary_key())
    }
    /// 无参数；返回全部字段负载字节数，不包含结构开销。
    pub const fn value_bytes(&self) -> usize {
        self.value_bytes
    }
    /// 参数为目标表元数据；验证原表名和全部列属性，供TableRecords原子添加前检查。
    pub(crate) fn matches_table(&self, table: &TableMeta) -> bool {
        self.table_name == table.name()
            && self.fields.len() == table.columns().len()
            && self
                .fields
                .iter()
                .zip(table.columns())
                .all(|(field, column)| field.column() == column)
    }
}
