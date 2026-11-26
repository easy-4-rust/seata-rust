use crate::{JavaString, MetadataError};
use std::sync::Arc;

/// Rust扩展：TableMeta返回的不可变、忽略大小写的有序主键名集合。
/// 对应 Java: TableMeta#getCaseInsensitivePKs使用的TreeSet(String.CASE_INSENSITIVE_ORDER)与unmodifiableSet。
/// 不计入原31对象；保留首次插入的大小写拼写及Java集合hash，不派生Rust的Eq/Hash。
#[derive(Debug, Clone)]
pub struct CaseInsensitivePkSet {
    names: Arc<[JavaString]>,
}

impl CaseInsensitivePkSet {
    /// 按names原插入顺序建立独立不可变集合，比较器相等时保留首个名称；null名称返回原NPE类别。
    /// 对应 Java: TreeSet#add；空集合允许建立，由TableMeta负责无主键业务错误。
    pub fn from_names(names: Vec<Option<JavaString>>) -> Result<Self, MetadataError> {
        let mut sorted: Vec<JavaString> = Vec::new();
        for name in names {
            let name = name.ok_or(MetadataError::NullReference("primary.columnName"))?;
            match sorted
                .binary_search_by(|current| current.java_compare_to_ignore_case(&name).cmp(&0))
            {
                Ok(_) => {}
                Err(index) => sorted.insert(index, name),
            }
        }
        Ok(Self {
            names: sorted.into(),
        })
    }

    /// 返回比较器去重后的主键数量；无参数，保留大小写相等而拼写不同的首次代表。
    /// 对应 Java: Set#size
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// 返回集合是否为空；无参数。
    /// 对应 Java: Set#isEmpty
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// 返回按JDK17比较器排序的只读名称切片；无参数，不提供能修改原集合的视图。
    /// 对应 Java: Collections.unmodifiableSet的有序iterator
    pub fn names(&self) -> &[JavaString] {
        &self.names
    }

    /// 检查nullable name是否存在；空集合查null为false，非空集合查null保留比较器NPE。
    /// 对应 Java: TreeSet#contains(Object)
    pub fn contains(&self, name: Option<&JavaString>) -> Result<bool, MetadataError> {
        if self.is_empty() {
            return Ok(false);
        }
        let name = name.ok_or(MetadataError::NullReference("primary.contains.name"))?;
        Ok(self
            .names
            .binary_search_by(|current| current.java_compare_to_ignore_case(name).cmp(&0))
            .is_ok())
    }

    /// 尝试加入nullable name始终返回UnsupportedOperationException，包括已存在名称或null。
    /// 对应 Java: Collections.UnmodifiableCollection#add
    pub fn add(&self, _name: Option<JavaString>) -> Result<bool, MetadataError> {
        Err(MetadataError::UnsupportedCollectionOperation("set.add"))
    }

    /// 尝试删除nullable name始终返回UnsupportedOperationException，包括空集合或缺失名称。
    /// 对应 Java: Collections.UnmodifiableCollection#remove
    pub fn remove(&self, _name: Option<&JavaString>) -> Result<bool, MetadataError> {
        Err(MetadataError::UnsupportedCollectionOperation("set.remove"))
    }

    /// 无参数，尝试清空始终返回UnsupportedOperationException，包括本来就为空的集合。
    /// 对应 Java: Collections.UnmodifiableCollection#clear
    pub fn clear(&self) -> Result<(), MetadataError> {
        Err(MetadataError::UnsupportedCollectionOperation("set.clear"))
    }

    /// 尝试加入names中的名称始终返回UnsupportedOperationException，包括空输入。
    /// 对应 Java: Collections.UnmodifiableCollection#addAll
    pub fn add_all(&self, _names: &[Option<JavaString>]) -> Result<bool, MetadataError> {
        Err(MetadataError::UnsupportedCollectionOperation("set.addAll"))
    }

    /// 比较other的成员，使用忽略大小写的集合包含关系；无额外状态修改。
    /// 对应 Java: AbstractSet#equals在同种主键集合之间的行为。
    pub fn java_equals(&self, other: &Self) -> bool {
        self.len() == other.len()
            && other.names.iter().all(|name| {
                self.names
                    .binary_search_by(|current| current.java_compare_to_ignore_case(name).cmp(&0))
                    .is_ok()
            })
    }

    /// 无参数，返回各原始拼写String.hashCode之和并按i32回绕；不改成忽略大小写的哈希。
    /// 对应 Java: AbstractSet#hashCode；原JDK集合相等时仍可能因拼写不同而hash不同。
    pub fn java_hash_code(&self) -> i32 {
        self.names
            .iter()
            .fold(0_i32, |sum, name| sum.wrapping_add(name.java_hash_code()))
    }
}
