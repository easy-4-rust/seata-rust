use crate::{ColumnMeta, JavaString, MetadataError};
use std::{
    fmt,
    sync::{Arc, RwLock},
};

type Slots = Arc<RwLock<Vec<Option<ColumnMeta>>>>;

/// Rust扩展：保留IndexMeta中列列表的nullable元素、共享身份和两种源集合模式。
/// 对应 Java: java.util.ArrayList及java.util.Arrays#asList的元数据使用契约；不计入31对象。
#[derive(Clone)]
pub struct ColumnList {
    state: Arc<RwLock<ListState>>,
    fixed: bool,
}

/// 同一列表内部状态；底层槽位在扩容前后保持独立身份，供原迭代语义使用。
#[derive(Clone)]
struct ListState {
    slots: Slots,
    size: usize,
    modification: i32,
    default_capacity: bool,
}

impl Default for ColumnList {
    fn default() -> Self {
        Self {
            state: Arc::new(RwLock::new(ListState {
                slots: Arc::new(RwLock::new(Vec::new())),
                size: 0,
                modification: 0,
                default_capacity: true,
            })),
            fixed: false,
        }
    }
}

impl fmt::Debug for ColumnList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColumnList")
            .field("identity", &Arc::as_ptr(&self.state))
            .field("fixed", &self.fixed)
            .finish()
    }
}

impl ColumnList {
    /// 用values建立可变长度列表，保留各ColumnMeta的原共享引用；返回独立列表。
    /// 对应 Java: ArrayList(Collection)
    pub fn from_values(values: Vec<Option<ColumnMeta>>) -> Self {
        Self::build(values, false)
    }

    /// 用values建立固定长度但可set的列表，保留nullable列引用；返回独立列表。
    /// 对应 Java: Arrays#asList
    pub fn fixed(values: Vec<Option<ColumnMeta>>) -> Self {
        Self::build(values, true)
    }

    fn build(values: Vec<Option<ColumnMeta>>, fixed: bool) -> Self {
        Self {
            state: Arc::new(RwLock::new(ListState {
                size: values.len(),
                slots: Arc::new(RwLock::new(values)),
                modification: 0,
                default_capacity: false,
            })),
            fixed,
        }
    }

    fn capture(&self) -> Result<ListState, MetadataError> {
        Ok(self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("ColumnList"))?
            .clone())
    }

    fn slot(slots: &Slots, index: usize) -> Result<Option<ColumnMeta>, MetadataError> {
        slots
            .read()
            .map_err(|_| MetadataError::LockPoisoned("ColumnList slots"))?
            .get(index)
            .cloned()
            .ok_or(MetadataError::ConcurrentModification)
    }

    fn check_version(&self, expected: i32) -> Result<(), MetadataError> {
        if self.capture()?.modification == expected {
            Ok(())
        } else {
            Err(MetadataError::ConcurrentModification)
        }
    }

    fn valid_index(&self, index: i32, size: usize) -> Result<usize, MetadataError> {
        usize::try_from(index)
            .ok()
            .filter(|value| *value < size)
            .ok_or(MetadataError::ListIndexOutOfBounds {
                index,
                size,
                array: self.fixed,
            })
    }

    /// 判断other是否为同一列表句柄；无副作用，不比较元素。
    /// 对应 Java: 列表引用的==
    pub fn same_reference(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    /// 返回当前长度，无参数；锁错误原样返回。
    /// 对应 Java: List#size
    pub fn len(&self) -> Result<usize, MetadataError> {
        Ok(self.capture()?.size)
    }

    /// 判断当前列表是否为空，无参数；锁错误原样返回。
    /// 对应 Java: List#isEmpty
    pub fn is_empty(&self) -> Result<bool, MetadataError> {
        Ok(self.len()? == 0)
    }

    /// 返回index处的nullable列共享引用；下标与锁错误明确返回。
    /// 对应 Java: List#get
    pub fn get(&self, index: i32) -> Result<Option<ColumnMeta>, MetadataError> {
        let state = self
            .state
            .read()
            .map_err(|_| MetadataError::LockPoisoned("ColumnList"))?;
        Self::slot(&state.slots, self.valid_index(index, state.size)?)
    }

    /// 在index处替换value并返回原nullable列；set不属于结构修改。
    /// 对应 Java: List#set
    pub fn set(
        &self,
        index: i32,
        value: Option<ColumnMeta>,
    ) -> Result<Option<ColumnMeta>, MetadataError> {
        let previous = {
            let state = self
                .state
                .read()
                .map_err(|_| MetadataError::LockPoisoned("ColumnList"))?;
            let index = self.valid_index(index, state.size)?;
            let mut slots = state
                .slots
                .write()
                .map_err(|_| MetadataError::LockPoisoned("ColumnList slots"))?;
            std::mem::replace(&mut slots[index], value)
        };
        // 不在集合锁内释放被替换列及其用户对象。
        Ok(previous)
    }

    /// 追加nullable value；固定长度列表明确拒绝，成功返回()。
    /// 对应 Java: ArrayList#add及Arrays.asList的UnsupportedOperationException
    pub fn push(&self, value: Option<ColumnMeta>) -> Result<(), MetadataError> {
        if self.fixed {
            return Err(MetadataError::UnsupportedListOperation("add"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| MetadataError::LockPoisoned("ColumnList"))?;
        if state.size >= i32::MAX as usize {
            return Err(MetadataError::CollectionCapacity);
        }
        let capacity = state
            .slots
            .read()
            .map_err(|_| MetadataError::LockPoisoned("ColumnList slots"))?
            .len();
        if state.size == capacity {
            let next = if state.default_capacity {
                10
            } else {
                capacity.saturating_add(capacity / 2).max(state.size + 1)
            };
            let mut replacement = state
                .slots
                .read()
                .map_err(|_| MetadataError::LockPoisoned("ColumnList slots"))?
                .clone();
            replacement.resize_with(next.min(i32::MAX as usize), || None);
            state.slots = Arc::new(RwLock::new(replacement));
            state.default_capacity = false;
        }
        let slots = Arc::clone(&state.slots);
        slots
            .write()
            .map_err(|_| MetadataError::LockPoisoned("ColumnList slots"))?[state.size] = value;
        state.size += 1;
        state.modification = state.modification.wrapping_add(1);
        Ok(())
    }

    /// 删除index处元素并返回其nullable列引用；固定长度列表不先做下标检查。
    /// 对应 Java: ArrayList#remove及AbstractList#remove
    pub fn remove(&self, index: i32) -> Result<Option<ColumnMeta>, MetadataError> {
        if self.fixed {
            return Err(MetadataError::UnsupportedListOperation("remove"));
        }
        let removed = {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("ColumnList"))?;
            let index = self.valid_index(index, state.size)?;
            let buffer = Arc::clone(&state.slots);
            let mut slots = buffer
                .write()
                .map_err(|_| MetadataError::LockPoisoned("ColumnList slots"))?;
            let removed = slots[index].take();
            for position in index..state.size - 1 {
                slots[position] = slots[position + 1].take();
            }
            state.size -= 1;
            state.modification = state.modification.wrapping_add(1);
            removed
        };
        Ok(removed)
    }

    /// 清空列表，无参数；固定空列表成功，非空固定列表拒绝；释放旧列前先释放锁。
    /// 对应 Java: ArrayList#clear及AbstractList#removeRange
    pub fn clear(&self) -> Result<(), MetadataError> {
        if self.fixed {
            return if self.is_empty()? {
                Ok(())
            } else {
                Err(MetadataError::UnsupportedListOperation("clear"))
            };
        }
        let removed = {
            let mut state = self
                .state
                .write()
                .map_err(|_| MetadataError::LockPoisoned("ColumnList"))?;
            let buffer = Arc::clone(&state.slots);
            let mut slots = buffer
                .write()
                .map_err(|_| MetadataError::LockPoisoned("ColumnList slots"))?;
            let removed: Vec<_> = slots[..state.size]
                .iter_mut()
                .filter_map(Option::take)
                .collect();
            state.size = 0;
            state.modification = state.modification.wrapping_add(1);
            removed
        };
        drop(removed);
        Ok(())
    }

    fn element_equals(
        left: Option<ColumnMeta>,
        right: Option<ColumnMeta>,
    ) -> Result<bool, MetadataError> {
        match (left, right) {
            (None, None) => Ok(true),
            (Some(left), Some(right)) => left.java_equals(Some(&right)),
            _ => Ok(false),
        }
    }

    fn has_next(&self, cursor: usize) -> Result<bool, MetadataError> {
        Ok(cursor != self.len()?)
    }

    fn iterator_next(
        &self,
        cursor: usize,
        expected: i32,
    ) -> Result<Option<ColumnMeta>, MetadataError> {
        let state = self.capture()?;
        if !self.fixed && state.modification != expected {
            return Err(MetadataError::ConcurrentModification);
        }
        if cursor >= state.size {
            return Err(MetadataError::ConcurrentModification);
        }
        Self::slot(&state.slots, cursor)
    }

    /// 按原列表模式比较other；回调不持锁，保留实时槽位及原结构修改检测时机。
    /// 对应 Java: ArrayList#equals、AbstractList#equals
    pub fn java_equals(&self, other: &Self) -> Result<bool, MetadataError> {
        if self.same_reference(other) {
            return Ok(true);
        }
        let mine = self.capture()?;
        let theirs = other.capture()?;
        if !self.fixed && !other.fixed {
            let mut equal = mine.size == theirs.size;
            if equal {
                for cursor in 0..mine.size {
                    if !Self::element_equals(
                        Self::slot(&mine.slots, cursor)?,
                        Self::slot(&theirs.slots, cursor)?,
                    )? {
                        equal = false;
                        break;
                    }
                }
            }
            other.check_version(theirs.modification)?;
            self.check_version(mine.modification)?;
            return Ok(equal);
        }
        if !self.fixed {
            let mut equal = true;
            for cursor in 0..mine.size {
                if !other.has_next(cursor)?
                    || !Self::element_equals(
                        Self::slot(&mine.slots, cursor)?,
                        other.iterator_next(cursor, theirs.modification)?,
                    )?
                {
                    equal = false;
                    break;
                }
            }
            if equal {
                equal = !other.has_next(mine.size)?;
            }
            self.check_version(mine.modification)?;
            return Ok(equal);
        }
        let mut cursor = 0;
        while self.has_next(cursor)? && other.has_next(cursor)? {
            let left = self.iterator_next(cursor, mine.modification)?;
            let right = other.iterator_next(cursor, theirs.modification)?;
            cursor += 1;
            if !Self::element_equals(left, right)? {
                return Ok(false);
            }
        }
        Ok(!(self.has_next(cursor)? || other.has_next(cursor)?))
    }

    /// 返回原Java列表文本；无参数，null元素保留，迭代期间的用户回调在锁外。
    /// 对应 Java: AbstractCollection#toString
    pub fn java_to_string(&self) -> Result<JavaString, MetadataError> {
        let initial = self.capture()?;
        let mut cursor = 0;
        let mut units = vec![91];
        while self.has_next(cursor)? {
            let value = self.iterator_next(cursor, initial.modification)?;
            if cursor != 0 {
                units.extend(", ".encode_utf16());
            }
            cursor += 1;
            match value {
                Some(column) => units.extend(column.java_to_string()?.as_utf16()),
                None => units.extend("null".encode_utf16()),
            }
        }
        units.push(93);
        Ok(JavaString::from_utf16(units))
    }
}
