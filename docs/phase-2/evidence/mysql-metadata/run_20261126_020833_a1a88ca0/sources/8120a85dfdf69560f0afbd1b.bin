use crate::{
    CaseInsensitivePkSet, ColumnMeta, IndexMeta, IndexType, JavaString, LowerCaseLinkHashMap,
    MetadataError, MetadataLocale, PrimaryKeyMap, SqlServerTableMeta,
};
use crate::{ColumnUtils, EscapeHandlerFactory};
use std::{
    any::Any,
    fmt,
    sync::{Arc, RwLock},
};

/// 表元数据共享对象；保留原字段、两个共享Map及主键/更新列/相等与哈希行为。
/// 对应 Java: org.apache.seata.sqlparser.struct.TableMeta。
/// 当前PARTIAL：显式Locale注入、完整返回Map视图及getEscapePkNameList/SPI链仍需继续迁移。
#[derive(Clone)]
pub struct TableMeta {
    state: Arc<RwLock<TableMetaState>>,
    all_columns: LowerCaseLinkHashMap<ColumnMeta>,
    all_indexes: LowerCaseLinkHashMap<IndexMeta>,
    locale: MetadataLocale,
}

/// TableMeta私有标量状态；Map为独立final共享对象，不在此锁内调用用户值方法。
#[derive(Default)]
struct TableMetaState {
    table_name: Option<JavaString>,
    original_table_name: Option<JavaString>,
    case_sensitive: bool,
}

impl fmt::Debug for TableMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableMeta")
            .field("identity", &Arc::as_ptr(&self.state))
            .finish()
    }
}

impl TableMeta {
    /// 用locale建立表元数据，返回原初值null/null/false和两个空Map；宿主默认Locale启动尚未替代为伪默认值。
    /// 对应 Java: TableMeta无参构造的状态，Locale依赖由Rust显式注入。
    pub fn new(locale: MetadataLocale) -> Self {
        Self {
            state: Arc::new(RwLock::new(TableMetaState::default())),
            all_columns: LowerCaseLinkHashMap::new(locale.clone()),
            all_indexes: LowerCaseLinkHashMap::new(locale.clone()),
            locale,
        }
    }
    /// 无参数，返回nullable表名，保留原UTF16。
    /// 对应 Java: TableMeta#getTableName
    pub fn table_name(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("TableMeta"))?
            .table_name
            .clone())
    }
    /// 设置nullable table_name，返回()；不修改原表名或Map。
    /// 对应 Java: TableMeta#setTableName
    pub fn set_table_name(&self, table_name: Option<JavaString>) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("TableMeta"))?
            .table_name = table_name;
        Ok(())
    }
    /// 无参数，返回nullable原始表名。
    /// 对应 Java: TableMeta#getOriginalTableName
    pub fn original_table_name(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("TableMeta"))?
            .original_table_name
            .clone())
    }
    /// 设置nullable original_table_name，返回()；该字段不参与原equals/hashCode。
    /// 对应 Java: TableMeta#setOriginalTableName
    pub fn set_original_table_name(
        &self,
        original_table_name: Option<JavaString>,
    ) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("TableMeta"))?
            .original_table_name = original_table_name;
        Ok(())
    }
    /// 以调用时语言查找nullable col_name，返回nullable共享列对象。
    /// 对应 Java: TableMeta#getColumnMeta
    pub fn column_meta(
        &self,
        col_name: Option<&JavaString>,
    ) -> Result<Option<Arc<ColumnMeta>>, MetadataError> {
        self.all_columns.get(col_name)
    }
    /// 无参数，返回原final列Map的共享句柄；后续修改影响本表，完整Java Map视图尚未迁移。
    /// 对应 Java: TableMeta#getAllColumns
    pub fn all_columns(&self) -> LowerCaseLinkHashMap<ColumnMeta> {
        self.all_columns.clone()
    }
    /// 无参数，返回原final索引Map共享句柄，保留别名。
    /// 对应 Java: TableMeta#getAllIndexes
    pub fn all_indexes(&self) -> LowerCaseLinkHashMap<IndexMeta> {
        self.all_indexes.clone()
    }
    /// 无参数，返回大小写敏感标志；不改变Map的原忽略大小写查询行为。
    /// 对应 Java: TableMeta#isCaseSensitive
    pub fn is_case_sensitive(&self) -> Result<bool, MetadataError> {
        Ok(self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("TableMeta"))?
            .case_sensitive)
    }
    /// 设置case_sensitive并返回()；原equals忽略该标志，hashCode仍包含它。
    /// 对应 Java: TableMeta#setCaseSensitive
    pub fn set_case_sensitive(&self, case_sensitive: bool) -> Result<(), MetadataError> {
        self.state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("TableMeta"))?
            .case_sensitive = case_sensitive;
        Ok(())
    }
    /// 无参数，按原列插入顺序返回第一个YES自增列；无匹配返回None，null列保留NPE。
    /// 对应 Java: TableMeta#getAutoIncreaseColumn；原方法不要求自增列属于主键。
    pub fn auto_increase_column(&self) -> Result<Option<Arc<ColumnMeta>>, MetadataError> {
        let mut cursor = self.all_columns.entries()?;
        while let Some((_, col)) = cursor.next_entry()? {
            let col = col.ok_or(MetadataError::NullReference("allColumns.value"))?;
            if col.is_autoincrement()? {
                return Ok(Some(col));
            }
        }
        Ok(None)
    }
    fn primary_columns(
        &self,
        mut action: impl FnMut(ColumnMeta) -> Result<(), MetadataError>,
    ) -> Result<(), MetadataError> {
        let mut cursor = self.all_indexes.entries()?;
        while let Some((_, index)) = cursor.next_entry()? {
            let index = index.ok_or(MetadataError::NullReference("allIndexes.value"))?;
            if index
                .indextype()?
                .ok_or(MetadataError::NullReference("index.indextype"))?
                .value()
                == IndexType::PRIMARY.value()
            {
                let values = index
                    .values()?
                    .ok_or(MetadataError::NullReference("index.values"))?;
                values.for_each_by_iterator(|col| {
                    action(col.ok_or(MetadataError::NullReference("index.column"))?)
                })?;
            }
        }
        Ok(())
    }
    fn missing_primary_key(&self) -> Result<MetadataError, MetadataError> {
        let mut units = match self.table_name()? {
            Some(name) => name.as_utf16().to_vec(),
            None => "null".encode_utf16().collect(),
        };
        units.extend(" needs to contain the primary key.".encode_utf16());
        Ok(MetadataError::NotSupportYet(JavaString::from_utf16(units)))
    }
    /// 无参数，收集全部PRIMARY索引的列到独立主键Map；重复精确名称覆盖旧值，缺主键抛原异常。
    /// 对应 Java: TableMeta#getPrimaryKeyMap；null列名允许作为HashMap键。
    pub fn primary_key_map(&self) -> Result<PrimaryKeyMap<ColumnMeta>, MetadataError> {
        let mut pk = PrimaryKeyMap::default();
        self.primary_columns(|col| {
            pk.insert(col.column_name()?, Some(Arc::new(col)));
            Ok(())
        })?;
        if pk.is_empty() {
            return Err(self.missing_primary_key()?);
        }
        Ok(pk)
    }
    /// 无参数，返回原忽略大小写不可变主键集合；null列名按TreeSet抛NPE，缺主键保留原异常。
    /// 对应 Java: TableMeta#getCaseInsensitivePKs
    pub fn case_insensitive_pks(&self) -> Result<CaseInsensitivePkSet, MetadataError> {
        let result = CaseInsensitivePkSet::from_source(|add| {
            self.primary_columns(|col| add(col.column_name()?))
        })?;
        if result.is_empty() {
            return Err(self.missing_primary_key()?);
        }
        Ok(result)
    }
    /// 无参数，按原HashMap条目遍历顺序返回nullable主键名列表。
    /// 对应 Java: TableMeta#getPrimaryKeyOnlyName
    pub fn primary_key_only_name(&self) -> Result<Vec<Option<JavaString>>, MetadataError> {
        Ok(self.primary_key_map()?.keys())
    }

    /// 按原主键顺序输出用 db_type 方言转义的列名。
    /// 对应 Java: TableMeta#getEscapePkNameList；factory 提供注册的服务域。
    pub fn escape_pk_name_list(
        &self,
        db_type: Option<&JavaString>,
        factory: &EscapeHandlerFactory,
    ) -> Result<Vec<Option<JavaString>>, MetadataError> {
        ColumnUtils::add_escapes(self.primary_key_only_name()?, db_type, factory)
    }
    /// 无参数，按列Map插入顺序返回onUpdate列名，保留null名称及共享列后续状态。
    /// 对应 Java: TableMeta#getOnUpdateColumnsOnlyName
    pub fn on_update_columns_only_name(&self) -> Result<Vec<Option<JavaString>>, MetadataError> {
        let mut result = Vec::new();
        let mut cursor = self.all_columns.entries()?;
        while let Some((_, col)) = cursor.next_entry()? {
            let col = col.ok_or(MetadataError::NullReference("allColumns.value"))?;
            if col.is_on_update()? {
                result.push(col.column_name()?);
            }
        }
        Ok(result)
    }
    /// 判断nullable cols是否包含全部主键；先精确匹配，再按调用时Locale大写，原工具保留null元素。
    /// 对应 Java: TableMeta#containsPK；原注释称至少一个，但实际containsAll要求全部，按代码保留。
    pub fn contains_pk(&self, cols: Option<&[Option<JavaString>]>) -> Result<bool, MetadataError> {
        let Some(cols) = cols else {
            return Ok(false);
        };
        let pk = self.primary_key_only_name()?;
        if pk.is_empty() {
            return Ok(false);
        }
        if pk.iter().all(|name| cols.contains(name)) {
            return Ok(true);
        }
        let uppercase =
            |names: &[Option<JavaString>]| -> Result<Vec<Option<JavaString>>, MetadataError> {
                names
                    .iter()
                    .map(|name| name.as_ref().map(|n| self.locale.upper(n)).transpose())
                    .collect()
            };
        let columns = uppercase(cols)?;
        let primary = uppercase(&pk)?;
        Ok(primary.iter().all(|name| columns.contains(name)))
    }
    /// 比较nullable other，先比较表名再other的列Map/索引Map；忽略originalTableName和caseSensitive。
    /// 对应 Java: TableMeta#equals；接受 SQL Server 子类型的父类元数据。
    pub fn java_equals(&self, other: Option<&dyn Any>) -> Result<bool, MetadataError> {
        let Some(other) = other.and_then(|value| {
            value.downcast_ref::<Self>().or_else(|| {
                value
                    .downcast_ref::<SqlServerTableMeta>()
                    .map(|table| &**table)
            })
        }) else {
            return Ok(false);
        };
        if Arc::ptr_eq(&self.state, &other.state) {
            return Ok(true);
        }
        if other.table_name()? != self.table_name()? {
            return Ok(false);
        }
        if !other.all_columns.java_equals(&self.all_columns)? {
            return Ok(false);
        }
        other.all_indexes.java_equals(&self.all_indexes)
    }
    /// 无参数，返回本机稳定枚举身份上下文的原Java式哈希。
    /// 对应 Java: TableMeta#hashCode；保留原equals忽略而hash包含caseSensitive的不一致。
    pub fn java_hash_code(&self) -> Result<i32, MetadataError> {
        self.java_hash_code_with_enum_identity(|value| value.value() + 1)
    }
    /// 用enum_identity回放同一JVM枚举哈希上下文，动态列回调及标量尾读取保持原时点。
    /// 对应 Java: TableMeta#hashCode的Enum身份输入扩展，回调不得在表或Map锁内执行。
    pub fn java_hash_code_with_enum_identity(
        &self,
        mut enum_identity: impl FnMut(IndexType) -> i32,
    ) -> Result<i32, MetadataError> {
        let mut hash = self
            .table_name()?
            .as_ref()
            .map_or(0, JavaString::java_hash_code);
        hash = hash.wrapping_add(self.all_columns.java_hash_code(&mut enum_identity)?);
        hash = hash.wrapping_add(self.all_indexes.java_hash_code(&mut enum_identity)?);
        Ok(hash.wrapping_add(if self.is_case_sensitive()? {
            1231
        } else {
            1237
        }))
    }
}
