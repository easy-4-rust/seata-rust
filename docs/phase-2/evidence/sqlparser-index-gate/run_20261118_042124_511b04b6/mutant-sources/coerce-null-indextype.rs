use super::IndexType;
use crate::{ColumnList, JavaString, MetadataError};
use std::{
    any::Any,
    fmt,
    sync::{Arc, RwLock},
};

/// 索引元数据的共享句柄；保留nullable列列表及原equals/hashCode字段选择。
/// 对应 Java: org.apache.seata.sqlparser.struct.IndexMeta
#[derive(Clone, Default)]
pub struct IndexMeta {
    state: Arc<RwLock<IndexMetaState>>,
}

/// IndexMeta内部状态，不是额外Java对象。
#[derive(Clone)]
struct IndexMetaState {
    values: Option<ColumnList>,
    non_unique: bool,
    index_qualifier: Option<JavaString>,
    index_name: Option<JavaString>,
    type_code: i16,
    indextype: Option<IndexType>,
    asc_or_desc: Option<JavaString>,
    cardinality: i64,
    ordinal_position: i32,
}

impl Default for IndexMetaState {
    fn default() -> Self {
        Self {
            values: Some(ColumnList::default()),
            non_unique: false,
            index_qualifier: None,
            index_name: None,
            type_code: 0,
            indextype: None,
            asc_or_desc: None,
            cardinality: 0,
            ordinal_position: 0,
        }
    }
}

impl fmt::Debug for IndexMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexMeta")
            .field("identity", &Arc::as_ptr(&self.state))
            .finish()
    }
}

impl IndexMeta {
    fn capture(&self) -> Result<IndexMetaState, MetadataError> {
        Ok(self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .clone())
    }

    /// 返回nullable列列表共享句柄，无参数；保留后续set和元素修改，锁中毒返回错误。
    /// 对应 Java: IndexMeta#getValues
    pub fn values(&self) -> Result<Option<ColumnList>, MetadataError> {
        Ok(self.capture()?.values)
    }

    /// 用nullable values替换列列表，返回()；释放旧列表前先释放索引锁。
    /// 对应 Java: IndexMeta#setValues
    pub fn set_values(&self, values: Option<ColumnList>) -> Result<(), MetadataError> {
        let previous = {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?;
            std::mem::replace(&mut state.values, values)
        };
        drop(previous);
        Ok(())
    }

    /// 返回非唯一标志，无参数；锁中毒返回错误。
    /// 对应 Java: IndexMeta#isNonUnique
    pub fn is_non_unique(&self) -> Result<bool, MetadataError> {
        Ok(self.capture()?.non_unique)
    }

    /// 设置non_unique，成功返回()；锁中毒返回错误。
    /// 对应 Java: IndexMeta#setNonUnique
    pub fn set_non_unique(&self, non_unique: bool) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .non_unique = non_unique;
        Ok(())
    }

    /// 返回nullable索引限定名，无参数；锁中毒返回错误。
    /// 对应 Java: IndexMeta#getIndexQualifier
    pub fn index_qualifier(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.capture()?.index_qualifier)
    }

    /// 用index_qualifier设置原始nullable限定名，成功返回()；锁中毒返回错误。
    /// 对应 Java: IndexMeta#setIndexQualifier
    pub fn set_index_qualifier(
        &self,
        index_qualifier: Option<JavaString>,
    ) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .index_qualifier = index_qualifier;
        Ok(())
    }

    /// 返回nullable索引名，无参数；锁中毒返回错误。
    /// 对应 Java: IndexMeta#getIndexName
    pub fn index_name(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.capture()?.index_name)
    }

    /// 用index_name设置原始nullable索引名，成功返回()；锁中毒返回错误。
    /// 对应 Java: IndexMeta#setIndexName
    pub fn set_index_name(&self, index_name: Option<JavaString>) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .index_name = index_name;
        Ok(())
    }

    /// 返回JDBC short类型编号，无参数；与indextype枚举不同，锁中毒返回错误。
    /// 对应 Java: IndexMeta#getType
    pub fn r#type(&self) -> Result<i16, MetadataError> {
        Ok(self.capture()?.type_code)
    }

    /// 设置原short参数type，成功返回()；不缩小到u8，锁中毒返回错误。
    /// 对应 Java: IndexMeta#setType
    pub fn set_type(&self, r#type: i16) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .type_code = r#type;
        Ok(())
    }

    /// 返回nullable索引分类，无参数；锁中毒返回错误。
    /// 对应 Java: IndexMeta#getIndextype
    pub fn indextype(&self) -> Result<Option<IndexType>, MetadataError> {
        Ok(self.capture()?.indextype)
    }

    /// 设置nullable indextype，成功返回()；不把null替换为PRIMARY，锁中毒返回错误。
    /// 对应 Java: IndexMeta#setIndextype
    pub fn set_indextype(&self, indextype: Option<IndexType>) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .indextype = indextype;
        Ok(())
    }

    /// 返回nullable排序方向，无参数；锁中毒返回错误。
    /// 对应 Java: IndexMeta#getAscOrDesc
    pub fn asc_or_desc(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.capture()?.asc_or_desc)
    }

    /// 用asc_or_desc设置nullable排序方向，成功返回()；锁中毒返回错误。
    /// 对应 Java: IndexMeta#setAscOrDesc
    pub fn set_asc_or_desc(&self, asc_or_desc: Option<JavaString>) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .asc_or_desc = asc_or_desc;
        Ok(())
    }

    /// 返回原long基数，无参数；此字段不参与原equals/hashCode。
    /// 对应 Java: IndexMeta#getCardinality
    pub fn cardinality(&self) -> Result<i64, MetadataError> {
        Ok(self.capture()?.cardinality)
    }

    /// 设置cardinality，保留负数和64位边界；成功返回()，锁中毒返回错误。
    /// 对应 Java: IndexMeta#setCardinality
    pub fn set_cardinality(&self, cardinality: i64) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .cardinality = cardinality;
        Ok(())
    }

    /// 返回原列位置，无参数；锁中毒返回错误。
    /// 对应 Java: IndexMeta#getOrdinalPosition
    pub fn ordinal_position(&self) -> Result<i32, MetadataError> {
        Ok(self.capture()?.ordinal_position)
    }

    /// 设置ordinal_position，成功返回()；锁中毒返回错误。
    /// 对应 Java: IndexMeta#setOrdinalPosition
    pub fn set_ordinal_position(&self, ordinal_position: i32) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("IndexMeta"))?
            .ordinal_position = ordinal_position;
        Ok(())
    }

    /// 比较nullable动态other；同一引用先返回true，其他类型false，原null分类解引用返回错误。
    /// 对应 Java: IndexMeta#equals；忽略cardinality，先other.values.equals(self.values)，回调后读取其余字段。
    pub fn java_equals(&self, other: Option<&dyn Any>) -> Result<bool, MetadataError> {
        let Some(other) = other.and_then(|value| value.downcast_ref::<Self>()) else {
            return Ok(false);
        };
        if Arc::ptr_eq(&self.state, &other.state) {
            return Ok(true);
        }
        let theirs = other.values()?;
        let mine = self.values()?;
        let same_values = match (&theirs, &mine) {
            (None, None) => true,
            (Some(left), Some(right)) => left.java_equals(right)?,
            _ => false,
        };
        if !same_values {
            return Ok(false);
        }
        // 列表比较可调用用户对象并改变索引字段，不能提前缓存这些后续字段。
        let theirs = other.capture()?;
        let mine = self.capture()?;
        if theirs.non_unique != mine.non_unique
            || theirs.index_qualifier != mine.index_qualifier
            || theirs.index_name != mine.index_name
            || theirs.type_code != mine.type_code
        {
            return Ok(false);
        }
        let other_type = theirs
            .indextype
            .unwrap_or(IndexType::PRIMARY);
        let my_type = mine
            .indextype
            .unwrap_or(IndexType::PRIMARY);
        if other_type.value() != my_type.value() {
            return Ok(false);
        }
        Ok(theirs.asc_or_desc == mine.asc_or_desc
            && theirs.ordinal_position == mine.ordinal_position)
    }

    /// 返回原字段选择的本机Java式哈希，无参数；枚举身份标记在本机稳定，不承诺等于另一JVM的随机身份值。
    /// 对应 Java: IndexMeta#hashCode；忽略values和cardinality。
    pub fn java_hash_code(&self) -> Result<i32, MetadataError> {
        self.java_hash_code_with_enum_identity(|value| value.value() + 1)
    }

    /// 用enum_identity提供的真实枚举身份哈希评估结果；这是跨JVM差分上下文入口，锁错误返回错误。
    /// 对应 Java: IndexMeta#hashCode中的Objects.hashCode(indextype)；回调不在锁内。
    pub fn java_hash_code_with_enum_identity(
        &self,
        enum_identity: impl FnOnce(IndexType) -> i32,
    ) -> Result<i32, MetadataError> {
        let state = self.capture()?;
        let mut hash = if state.non_unique { 1231_i32 } else { 1237_i32 };
        hash = hash.wrapping_add(
            state
                .index_qualifier
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(
            state
                .index_name
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        hash = hash.wrapping_add(i32::from(state.type_code));
        hash = hash.wrapping_add(state.indextype.map_or(0, enum_identity));
        let state = self.capture()?;
        hash = hash.wrapping_add(
            state
                .asc_or_desc
                .as_ref()
                .map_or(0, JavaString::java_hash_code),
        );
        Ok(hash.wrapping_add(state.ordinal_position))
    }

    /// 返回原indexName/type/values文本，无参数；列表回调在锁外，字符串UTF16无损。
    /// 对应 Java: IndexMeta#toString
    pub fn java_to_string(&self) -> Result<JavaString, MetadataError> {
        let state = self.capture()?;
        let mut units: Vec<u16> = "indexName:".encode_utf16().collect();
        match state.index_name {
            Some(name) => units.extend(name.as_utf16()),
            None => units.extend("null".encode_utf16()),
        }
        units.extend("->type:".encode_utf16());
        units.extend(state.type_code.to_string().encode_utf16());
        units.extend("->values:".encode_utf16());
        match state.values {
            Some(values) => units.extend(values.java_to_string()?.as_utf16()),
            None => units.extend("null".encode_utf16()),
        }
        Ok(JavaString::from_utf16(units))
    }
}
