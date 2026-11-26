use crate::{JavaString, MetadataError};
use std::sync::Arc;

/// Rust扩展：TableMeta返回的不可变、忽略大小写的有序主键名集合。
/// 对应 Java: TableMeta#getCaseInsensitivePKs使用的TreeSet(String.CASE_INSENSITIVE_ORDER)与unmodifiableSet。
/// 不计入原31对象；保留首次插入的大小写拼写及Java集合hash，不派生Rust的Eq/Hash。
#[derive(Debug, Clone)]
pub struct CaseInsensitivePkSet {
    tree: Arc<PkTree>,
    names: Arc<[JavaString]>,
}

/// 原TreeMap插入所需的私有存储，不是新增迁移对象；索引代替Java节点引用。
#[derive(Debug, Default)]
struct PkTree {
    nodes: Vec<PkTreeNode>,
    root: Option<usize>,
}

/// 主键树内部节点；保留含非传递比较器的原红黑树形状。
#[derive(Debug)]
struct PkTreeNode {
    name: JavaString,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
    red: bool,
}

impl PkTree {
    fn insert(&mut self, name: JavaString) {
        let Some(mut current) = self.root else {
            self.nodes.push(PkTreeNode {
                name,
                parent: None,
                left: None,
                right: None,
                red: false,
            });
            self.root = Some(0);
            return;
        };
        loop {
            let difference = name.java_compare_to_ignore_case(&self.nodes[current].name);
            if difference == 0 {
                // TreeMap替换value但不替换已存在的key，因而保留首次拼写。
                return;
            }
            let next = if difference < 0 {
                self.nodes[current].left
            } else {
                self.nodes[current].right
            };
            if let Some(next) = next {
                current = next;
                continue;
            }
            let inserted = self.nodes.len();
            self.nodes.push(PkTreeNode {
                name,
                parent: Some(current),
                left: None,
                right: None,
                red: true,
            });
            if difference < 0 {
                self.nodes[current].left = Some(inserted);
            } else {
                self.nodes[current].right = Some(inserted);
            }
            self.fix_after_insertion(inserted);
            return;
        }
    }

    fn is_red(&self, node: Option<usize>) -> bool {
        node.is_some_and(|index| self.nodes[index].red)
    }

    fn rotate_left(&mut self, pivot: usize) {
        if let Some(right) = self.nodes[pivot].right {
            let inside = self.nodes[right].left;
            self.nodes[pivot].right = inside;
            if let Some(inside) = inside {
                self.nodes[inside].parent = Some(pivot);
            }
            let parent = self.nodes[pivot].parent;
            self.nodes[right].parent = parent;
            if let Some(parent) = parent {
                if self.nodes[parent].left == Some(pivot) {
                    self.nodes[parent].left = Some(right);
                } else {
                    self.nodes[parent].right = Some(right);
                }
            } else {
                self.root = Some(right);
            }
            self.nodes[right].left = Some(pivot);
            self.nodes[pivot].parent = Some(right);
        }
    }

    fn rotate_right(&mut self, pivot: usize) {
        if let Some(left) = self.nodes[pivot].left {
            let inside = self.nodes[left].right;
            self.nodes[pivot].left = inside;
            if let Some(inside) = inside {
                self.nodes[inside].parent = Some(pivot);
            }
            let parent = self.nodes[pivot].parent;
            self.nodes[left].parent = parent;
            if let Some(parent) = parent {
                if self.nodes[parent].right == Some(pivot) {
                    self.nodes[parent].right = Some(left);
                } else {
                    self.nodes[parent].left = Some(left);
                }
            } else {
                self.root = Some(left);
            }
            self.nodes[left].right = Some(pivot);
            self.nodes[pivot].parent = Some(left);
        }
    }

    fn fix_after_insertion(&mut self, mut node: usize) {
        // 保留原JDK17变色及旋转时点，比较器不传递时，树形决定查询和遍历的可观察结果。
        while let Some(mut parent) = self.nodes[node].parent {
            if !self.nodes[parent].red {
                break;
            }
            let Some(grandparent) = self.nodes[parent].parent else {
                break;
            };
            let parent_is_left = self.nodes[grandparent].left == Some(parent);
            let uncle = if parent_is_left {
                self.nodes[grandparent].right
            } else {
                self.nodes[grandparent].left
            };
            if self.is_red(uncle) {
                self.nodes[parent].red = false;
                if let Some(uncle) = uncle {
                    self.nodes[uncle].red = false;
                }
                self.nodes[grandparent].red = true;
                node = grandparent;
            } else if parent_is_left {
                if self.nodes[parent].right == Some(node) {
                    self.rotate_left(parent);
                    std::mem::swap(&mut node, &mut parent);
                }
                self.nodes[parent].red = false;
                self.nodes[grandparent].red = true;
                self.rotate_right(grandparent);
            } else {
                if self.nodes[parent].left == Some(node) {
                    self.rotate_right(parent);
                    std::mem::swap(&mut node, &mut parent);
                }
                self.nodes[parent].red = false;
                self.nodes[grandparent].red = true;
                self.rotate_left(grandparent);
            }
        }
        if let Some(root) = self.root {
            self.nodes[root].red = false;
        }
    }

    fn ordered_names(&self) -> Vec<JavaString> {
        let mut result = Vec::with_capacity(self.nodes.len());
        let mut stack = Vec::new();
        let mut next = self.root;
        loop {
            while let Some(index) = next {
                stack.push(index);
                next = self.nodes[index].left;
            }
            let Some(index) = stack.pop() else {
                break;
            };
            result.push(self.nodes[index].name.clone());
            next = self.nodes[index].right;
        }
        result
    }

    fn contains(&self, name: &JavaString) -> bool {
        let mut next = self.root;
        while let Some(index) = next {
            let difference = name.java_compare_to_ignore_case(&self.nodes[index].name);
            if difference == 0 {
                return true;
            }
            next = if difference < 0 {
                self.nodes[index].left
            } else {
                self.nodes[index].right
            };
        }
        false
    }
}

impl CaseInsensitivePkSet {
    /// 按names原插入顺序建立独立不可变集合，比较器相等时保留首个名称；null名称返回原NPE类别。
    /// 对应 Java: TreeSet#add；空集合允许建立，由TableMeta负责无主键业务错误。
    pub fn from_names(names: Vec<Option<JavaString>>) -> Result<Self, MetadataError> {
        let mut tree = PkTree::default();
        for name in names {
            let name = name.ok_or(MetadataError::NullReference("primary.columnName"))?;
            tree.insert(name);
        }
        Ok(Self {
            names: tree.ordered_names().into(),
            tree: Arc::new(tree),
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

    /// 返回原JDK17树形中序遍历的只读名称切片；无参数，不把非传递比较器强制排序。
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
        Ok(self.tree.contains(name))
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
        Ok(false)
    }

    /// 比较other的成员，使用忽略大小写的集合包含关系；无额外状态修改。
    /// 对应 Java: AbstractSet#equals在同种主键集合之间的行为。
    pub fn java_equals(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.tree, &other.tree)
            || (self.len() == other.len()
                && other.names.iter().all(|name| self.tree.contains(name)))
    }

    /// 无参数，返回各原始拼写String.hashCode之和并按i32回绕；不改成忽略大小写的哈希。
    /// 对应 Java: AbstractSet#hashCode；原JDK集合相等时仍可能因拼写不同而hash不同。
    pub fn java_hash_code(&self) -> i32 {
        self.names
            .iter()
            .fold(0_i32, |sum, name| sum.wrapping_add(name.java_hash_code()))
    }
}
