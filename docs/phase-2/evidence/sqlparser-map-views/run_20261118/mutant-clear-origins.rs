use crate::{
    IndexType, JavaString, MetadataEntryCursor, MetadataEntrySet, MetadataError, MetadataLiveEntry,
    MetadataLocale, MetadataMapValue, MetadataValues, PrimaryKeyMap,
};
use std::{
    fmt,
    sync::{Arc, RwLock},
};

/// 元数据使用的共享忽略大小写插入顺序Map；保留调用时Locale、nullable值与重入回调顺序。
/// 对应 Java: org.apache.seata.common.util.LowerCaseLinkHashMap；当前提供TableMeta依赖路径，完整Map视图/默认方法仍待迁移。
pub struct LowerCaseLinkHashMap<V: MetadataMapValue> {
    state: Arc<RwLock<MapState<V>>>,
    locale: MetadataLocale,
}

/// Map内部两个原LinkedHashMap的状态；删除留下私有墓碑供已有游标保持next引用。
struct MapState<V: MetadataMapValue> {
    nodes: Vec<Option<MetadataLiveEntry<V>>>,
    origins: Vec<(JavaString, JavaString)>,
    size: usize,
    modification: i32,
}
/// 私有游标单步结果；对应原next链接、Entry键和值，不是独立源对象。
type CursorStep<V> = (Option<usize>, MetadataLiveEntry<V>);

impl<V: MetadataMapValue> MapState<V> {
    fn detach(&mut self, index: usize) -> Option<MetadataLiveEntry<V>> {
        let detached = self.nodes.get_mut(index)?.take()?;
        self.size -= 1;
        self.modification = self.modification.wrapping_add(1);
        Some(detached)
    }
}

impl<V: MetadataMapValue> Clone for LowerCaseLinkHashMap<V> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            locale: self.locale.clone(),
        }
    }
}
impl<V: MetadataMapValue> fmt::Debug for LowerCaseLinkHashMap<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LowerCaseLinkHashMap")
            .field("identity", &Arc::as_ptr(&self.state))
            .finish()
    }
}

impl<V: MetadataMapValue> LowerCaseLinkHashMap<V> {
    /// 用locale建立空共享Map并返回；不读取未验证的系统默认Locale，也不伪装已完成全部Java Map接口。
    /// 对应 Java: LowerCaseLinkHashMap构造及其String.toLowerCase依赖的显式注入。
    pub fn new(locale: MetadataLocale) -> Self {
        Self {
            state: Arc::new(RwLock::new(MapState {
                nodes: Vec::new(),
                origins: Vec::new(),
                size: 0,
                modification: 0,
            })),
            locale,
        }
    }
    /// 无参数，返回当前键数量；含null值的条目也计数。
    /// 对应 Java: LowerCaseLinkHashMap#size
    pub fn len(&self) -> Result<usize, MetadataError> {
        Ok(self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?
            .size)
    }
    /// 无参数，返回Map是否为空，失败不伪造成空Map。
    /// 对应 Java: LowerCaseLinkHashMap#isEmpty
    pub fn is_empty(&self) -> Result<bool, MetadataError> {
        Ok(self.len()? == 0)
    }
    /// 以当前locale查询可空key，返回nullable共享值；null查询返回None。
    /// 对应 Java: LowerCaseLinkHashMap#get(Object)
    pub fn get(&self, key: Option<&JavaString>) -> Result<Option<Arc<V>>, MetadataError> {
        let Some(key) = key else {
            return Ok(None);
        };
        self.raw_get(&self.locale.lower(key)?)
            .map(|(_, value)| value)
    }
    /// 以当前locale查询可空key是否存在；null为false，含null值的条目仍存在。
    /// 对应 Java: LowerCaseLinkHashMap#containsKey
    pub fn contains_key(&self, key: Option<&JavaString>) -> Result<bool, MetadataError> {
        let Some(key) = key else {
            return Ok(false);
        };
        self.raw_get(&self.locale.lower(key)?)
            .map(|(exists, _)| exists)
    }
    fn raw_get(&self, key: &JavaString) -> Result<(bool, Option<Arc<V>>), MetadataError> {
        match self.find_entry(key)? {
            Some((_, entry)) => Ok((true, entry.value()?)),
            None => Ok((false, None)),
        }
    }
    /// 查找规范化的原始 key，返回节点及其槽位。
    /// 对应 Java: LinkedHashMap#getNode。
    pub(crate) fn find_entry(
        &self,
        key: &JavaString,
    ) -> Result<Option<(usize, MetadataLiveEntry<V>)>, MetadataError> {
        let state = self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
        Ok(state
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_ref().map(|node| (index, node)))
            .find(|(_, node)| node.key() == key)
            .map(|(index, node)| (index, node.clone())))
    }
    /// 插入nullable value并返回原值；key为null保留NPE，替换既有key不改变插入位置或结构版本。
    /// 对应 Java: LowerCaseLinkHashMap#put，两次lower调用分别读取当前locale。
    pub fn put(
        &self,
        key: Option<JavaString>,
        value: Option<Arc<V>>,
    ) -> Result<Option<Arc<V>>, MetadataError> {
        let key = key.ok_or(MetadataError::NullReference("map.key"))?;
        let origin_key = self.locale.lower(&key)?;
        {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
            if let Some((_, origin)) = state.origins.iter_mut().find(|(k, _)| *k == origin_key) {
                *origin = key.clone();
            } else {
                state.origins.push((origin_key, key.clone()));
            }
        }
        let normalized = self.locale.lower(&key)?;
        let previous = {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
            if let Some(node) = state
                .nodes
                .iter()
                .flatten()
                .find(|n| n.key() == &normalized)
            {
                node.set_value(value)?
            } else {
                state
                    .nodes
                    .push(Some(MetadataLiveEntry::new(normalized, value)));
                state.size += 1;
                state.modification = state.modification.wrapping_add(1);
                None
            }
        };
        Ok(previous)
    }
    /// 删除nullable key并返回原值；非字符串/null入口不修改Map，旧值返回后在锁外释放。
    /// 对应 Java: LowerCaseLinkHashMap#remove(Object)
    pub fn remove(&self, key: Option<&JavaString>) -> Result<Option<Arc<V>>, MetadataError> {
        let Some(key) = key else {
            return Ok(None);
        };
        let normalized = self.locale.lower(key)?;
        {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
            state.origins.retain(|(k, _)| *k != normalized);
        }
        let normalized = self.locale.lower(key)?;
        let detached = {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
            let index = state
                .nodes
                .iter()
                .position(|slot| slot.as_ref().is_some_and(|node| node.key() == &normalized));
            index.and_then(|index| state.detach(index))
        };
        detached.map_or(Ok(None), |entry| entry.value())
    }
    /// 无参数，清空两个映射；即便本来为空也增加目标LinkedHashMap结构版本，旧值在锁外释放。
    /// 对应 Java: LowerCaseLinkHashMap#clear及LinkedHashMap继承的HashMap.clear。
    pub fn clear(&self) -> Result<(), MetadataError> {
        self.clear_maps(true)
    }
    /// 仅清空目标 Map，原始键记录不变。
    /// 对应 Java: targetMap.clear，由条目或值视图触发。
    pub(crate) fn clear_target(&self) -> Result<(), MetadataError> {
        self.clear_maps(true)
    }
    fn clear_maps(&self, clear_origins: bool) -> Result<(), MetadataError> {
        let old = {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
            let old = std::mem::take(&mut state.nodes);
            if clear_origins {
                state.origins.clear();
            }
            state.size = 0;
            state.modification = state.modification.wrapping_add(1);
            old
        };
        drop(old);
        Ok(())
    }
    /// 返回实时条目视图，视图删除只修改目标 Map。
    /// 对应 Java: LowerCaseLinkHashMap#entrySet。
    pub fn entry_set(&self) -> MetadataEntrySet<V> {
        MetadataEntrySet(self.clone())
    }
    /// 返回目标 Map 的实时值视图。
    /// 对应 Java: LowerCaseLinkHashMap#values。
    pub fn values(&self) -> MetadataValues<V> {
        MetadataValues(self.clone())
    }
    /// 返回按原 JDK HashSet 遍历顺序独立复制的原始拼写集合。
    /// 对应 Java: LowerCaseLinkHashMap#keySet；Rust 返回集合顺序的快照。
    pub fn key_snapshot(&self) -> Result<Vec<JavaString>, MetadataError> {
        let origins: Vec<_> = self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?
            .origins
            .iter()
            .map(|(_, original)| original.clone())
            .collect();
        let mut keys = PrimaryKeyMap::<()>::for_key_set(origins.len());
        for name in origins {
            keys.insert(Some(name), None);
        }
        Ok(keys.keys().into_iter().flatten().collect())
    }
    /// 建立保留结构版本的实时条目游标。
    /// 对应 Java: targetMap.entrySet().iterator()。
    pub fn entries(&self) -> Result<MetadataEntryCursor<V>, MetadataError> {
        let state = self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
        Ok(MetadataEntryCursor::new(
            self.clone(),
            state.modification,
            state.nodes.iter().position(Option::is_some),
        ))
    }
    /// 校验version并读取index节点及其原next链接；回调及用户值释放均不在锁内。
    /// 对应 Java: LinkedHashIterator#nextNode
    pub(crate) fn cursor_next(
        &self,
        version: i32,
        index: usize,
    ) -> Result<CursorStep<V>, MetadataError> {
        let state = self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
        if state.modification != version {
            return Err(MetadataError::ConcurrentModification);
        }
        let node = state.nodes.get(index).and_then(Option::as_ref).ok_or(
            MetadataError::InvalidCompatibilityData("metadata cursor node"),
        )?;
        let next = state
            .nodes
            .iter()
            .enumerate()
            .skip(index + 1)
            .find(|(_, n)| n.is_some())
            .map(|(i, _)| i);
        Ok((next, node.clone()))
    }
    /// 对比游标版本，显式 next 即使到达末尾仍优先检查结构修改。
    /// 对应 Java: LinkedHashIterator#nextNode。
    pub(crate) fn cursor_check(&self, version: i32) -> Result<(), MetadataError> {
        if self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?
            .modification
            != version
        {
            return Err(MetadataError::ConcurrentModification);
        }
        Ok(())
    }
    /// 删除游标最近返回的目标节点，返回删除后的版本。
    /// 对应 Java: LinkedHashIterator#remove。
    pub(crate) fn cursor_remove(&self, version: i32, index: usize) -> Result<i32, MetadataError> {
        let (entry, version_after) = {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
            if state.modification != version {
                return Err(MetadataError::ConcurrentModification);
            }
            let entry = state.detach(index).ok_or(MetadataError::IteratorState)?;
            (entry, state.modification)
        };
        drop(entry);
        Ok(version_after)
    }
    /// 比较回调完成后移除原节点，避免对后来复用的槽位执行删除。
    /// 对应 Java: LinkedHashMap.LinkedEntrySet#remove 的目标节点路径。
    pub(crate) fn remove_entry_if_same(
        &self,
        index: usize,
        entry: &MetadataLiveEntry<V>,
    ) -> Result<bool, MetadataError> {
        let detached = {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("LowerCaseLinkHashMap"))?;
            if state
                .nodes
                .get(index)
                .and_then(Option::as_ref)
                .is_some_and(|current| current.same_node(entry))
            {
                state.detach(index)
            } else {
                None
            }
        };
        Ok(detached.is_some())
    }
    /// 比较other的目标原始键值Map；不重新lower键，不比较保存原拼写的第二Map。
    /// 对应 Java: LowerCaseLinkHashMap#equals及AbstractMap#equals，NPE/CCE按原规则返回false。
    pub fn java_equals(&self, other: &Self) -> Result<bool, MetadataError> {
        if Arc::ptr_eq(&self.state, &other.state) {
            return Ok(true);
        }
        if self.len()? != other.len()? {
            return Ok(false);
        }
        let mut cursor = self.entries()?;
        while let Some((key, value)) = cursor.next_entry()? {
            let (exists, theirs) = other.raw_get(&key)?;
            let equal = match value {
                Some(value) => value.metadata_equals(theirs.as_deref()),
                None => Ok(theirs.is_none() && exists),
            };
            match equal {
                Ok(true) => {}
                Ok(false) => return Ok(false),
                Err(error) => {
                    if error.caught_by_map_equals() {
                        return Ok(false);
                    }
                    return Err(error);
                }
            }
        }
        Ok(true)
    }
    /// 用enum_identity计算原Map哈希；保留Objects.hash(targetMap)额外加31，而非直接返回entry和。
    /// 对应 Java: LowerCaseLinkHashMap#hashCode
    pub fn java_hash_code(
        &self,
        enum_identity: &mut dyn FnMut(IndexType) -> i32,
    ) -> Result<i32, MetadataError> {
        let mut hash = 0_i32;
        let mut cursor = self.entries()?;
        while let Some((key, value)) = cursor.next_entry()? {
            let value_hash = match value {
                Some(value) => value.metadata_hash(enum_identity)?,
                None => 0,
            };
            hash = hash.wrapping_add(key.java_hash_code() ^ value_hash);
        }
        Ok(hash.wrapping_add(31))
    }
}
