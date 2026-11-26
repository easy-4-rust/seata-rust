use crate::JavaString;
use std::{cmp::Ordering, sync::Arc};

/// Rust扩展：保留TableMeta主键结果所用原JDK17 HashMap的分桶、扩容及树桶遍历顺序。
/// 对应 Java: TableMeta#getPrimaryKeyMap的HashMap<String,ColumnMeta>；当前公开基础读写，完整Map视图/删除仍待迁移。
pub struct PrimaryKeyMap<V> {
    table: Vec<Option<usize>>,
    nodes: Vec<HashNode<V>>,
    size: usize,
    threshold: usize,
}

/// 主键Map私有节点；树链接与桶内next链接分开保存，不能用有序Map替代HashMap遍历。
struct HashNode<V> {
    key: Option<JavaString>,
    value: Option<Arc<V>>,
    hash: i32,
    next: Option<usize>,
    prev: Option<usize>,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
    red: bool,
    tree: bool,
}

impl<V> Default for PrimaryKeyMap<V> {
    fn default() -> Self {
        Self {
            table: Vec::new(),
            nodes: Vec::new(),
            size: 0,
            threshold: 0,
        }
    }
}

impl<V> PrimaryKeyMap<V> {
    fn spread(key: Option<&JavaString>) -> i32 {
        let hash = key.map_or(0, JavaString::java_hash_code) as u32;
        (hash ^ (hash >> 16)) as i32
    }
    fn find(&self, key: Option<&JavaString>) -> Option<usize> {
        if self.table.is_empty() {
            return None;
        }
        let hash = Self::spread(key);
        let mut next = self.table[(hash as u32 as usize) & (self.table.len() - 1)];
        while let Some(index) = next {
            let node = &self.nodes[index];
            if node.hash == hash && node.key.as_ref() == key {
                return Some(index);
            }
            next = node.next;
        }
        None
    }
    /// 无参数，返回含null键/值的主键条目数量。
    /// 对应 Java: HashMap#size
    pub fn len(&self) -> usize {
        self.size
    }
    /// 无参数，返回结果是否为空。
    /// 对应 Java: HashMap#isEmpty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
    /// 查询nullable key是否存在，区分缺失条目和存在的null值。
    /// 对应 Java: HashMap#containsKey
    pub fn contains_key(&self, key: Option<&JavaString>) -> bool {
        self.find(key).is_some()
    }
    /// 查询nullable key，返回nullable共享value；不修改桶或顺序。
    /// 对应 Java: HashMap#get
    pub fn get(&self, key: Option<&JavaString>) -> Option<Arc<V>> {
        self.find(key).and_then(|i| self.nodes[i].value.clone())
    }
    /// 无参数，按原HashMap桶及next链接顺序返回nullable键快照，保留树根前移的结果。
    /// 对应 Java: HashMap.entrySet迭代并读取Entry#getKey。
    pub fn keys(&self) -> Vec<Option<JavaString>> {
        let mut result = Vec::with_capacity(self.size);
        for head in &self.table {
            let mut next = *head;
            while let Some(index) = next {
                result.push(self.nodes[index].key.clone());
                next = self.nodes[index].next;
            }
        }
        result
    }
    /// 插入nullable key/value并返回原值；相等键替换值但保留原key及桶位置。
    /// 对应 Java: HashMap#put；默认容量/阈值、树化和分裂遵循固定JDK17。
    pub fn insert(&mut self, key: Option<JavaString>, value: Option<Arc<V>>) -> Option<Arc<V>> {
        if let Some(index) = self.find(key.as_ref()) {
            return std::mem::replace(&mut self.nodes[index].value, value);
        }
        if self.table.is_empty() {
            self.resize();
        }
        let hash = Self::spread(key.as_ref());
        let bucket = (hash as u32 as usize) & (self.table.len() - 1);
        let head = self.table[bucket];
        let inserted = self.nodes.len();
        self.nodes.push(HashNode {
            key,
            value,
            hash,
            next: None,
            prev: None,
            parent: None,
            left: None,
            right: None,
            red: false,
            tree: false,
        });
        match head {
            None => self.table[bucket] = Some(inserted),
            Some(head) if self.nodes[head].tree => self.insert_tree(bucket, head, inserted),
            Some(mut last) => {
                let mut count = 0;
                while let Some(next) = self.nodes[last].next {
                    last = next;
                    count += 1;
                }
                self.nodes[last].next = Some(inserted);
                if count >= 7 {
                    self.treeify(bucket);
                }
            }
        }
        self.size += 1;
        if self.size > self.threshold {
            self.resize();
        }
        None
    }
    fn compare(&self, a: usize, b: usize) -> Ordering {
        self.nodes[a].hash.cmp(&self.nodes[b].hash).then_with(|| {
            match (&self.nodes[a].key, &self.nodes[b].key) {
                (Some(a), Some(b)) => a.as_utf16().cmp(b.as_utf16()),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
        })
    }
    fn rotate_left(&mut self, root: &mut usize, pivot: usize) {
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
                *root = right;
                self.nodes[right].red = false;
            }
            self.nodes[right].left = Some(pivot);
            self.nodes[pivot].parent = Some(right);
        }
    }
    fn rotate_right(&mut self, root: &mut usize, pivot: usize) {
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
                *root = left;
                self.nodes[left].red = false;
            }
            self.nodes[left].right = Some(pivot);
            self.nodes[pivot].parent = Some(left);
        }
    }
    fn balance(&mut self, mut root: usize, mut node: usize) -> usize {
        self.nodes[node].red = true;
        loop {
            let Some(mut parent) = self.nodes[node].parent else {
                self.nodes[node].red = false;
                return node;
            };
            if !self.nodes[parent].red {
                return root;
            }
            let Some(grand) = self.nodes[parent].parent else {
                return root;
            };
            let left = self.nodes[grand].left == Some(parent);
            let uncle = if left {
                self.nodes[grand].right
            } else {
                self.nodes[grand].left
            };
            if uncle.is_some_and(|i| self.nodes[i].red) {
                self.nodes[parent].red = false;
                if let Some(uncle) = uncle {
                    self.nodes[uncle].red = false;
                }
                self.nodes[grand].red = true;
                node = grand;
            } else if left {
                if self.nodes[parent].right == Some(node) {
                    self.rotate_left(&mut root, parent);
                    std::mem::swap(&mut node, &mut parent);
                }
                self.nodes[parent].red = false;
                self.nodes[grand].red = true;
                self.rotate_right(&mut root, grand);
            } else {
                if self.nodes[parent].left == Some(node) {
                    self.rotate_right(&mut root, parent);
                    std::mem::swap(&mut node, &mut parent);
                }
                self.nodes[parent].red = false;
                self.nodes[grand].red = true;
                self.rotate_left(&mut root, grand);
            }
        }
    }
    fn move_root(&mut self, bucket: usize, root: usize) {
        let first = self.table[bucket];
        if first == Some(root) {
            return;
        }
        let previous = self.nodes[root].prev;
        let next = self.nodes[root].next;
        if let Some(next) = next {
            self.nodes[next].prev = previous;
        }
        if let Some(previous) = previous {
            self.nodes[previous].next = next;
        }
        if let Some(first) = first {
            self.nodes[first].prev = Some(root);
        }
        self.nodes[root].next = first;
        self.nodes[root].prev = None;
        self.table[bucket] = Some(root);
    }
    fn treeify(&mut self, bucket: usize) {
        if self.table.len() < 64 {
            self.resize();
            return;
        }
        let mut next = self.table[bucket];
        let mut root = None;
        let mut previous = None;
        while let Some(index) = next {
            next = self.nodes[index].next;
            self.nodes[index].tree = true;
            self.nodes[index].prev = previous;
            previous = Some(index);
            self.nodes[index].parent = None;
            self.nodes[index].left = None;
            self.nodes[index].right = None;
            if let Some(current_root) = root {
                let mut parent = current_root;
                loop {
                    let left = self.compare(index, parent) != Ordering::Greater;
                    let child = if left {
                        self.nodes[parent].left
                    } else {
                        self.nodes[parent].right
                    };
                    if let Some(child) = child {
                        parent = child;
                    } else {
                        self.nodes[index].parent = Some(parent);
                        if left {
                            self.nodes[parent].left = Some(index);
                        } else {
                            self.nodes[parent].right = Some(index);
                        }
                        break;
                    }
                }
                root = Some(self.balance(current_root, index));
            } else {
                self.nodes[index].red = false;
                root = Some(index);
            }
        }
        if let Some(root) = root {
            self.move_root(bucket, root);
        }
    }
    fn insert_tree(&mut self, bucket: usize, head: usize, index: usize) {
        let mut root = head;
        while let Some(parent) = self.nodes[root].parent {
            root = parent;
        }
        let mut parent = root;
        loop {
            let left = self.compare(index, parent) != Ordering::Greater;
            let child = if left {
                self.nodes[parent].left
            } else {
                self.nodes[parent].right
            };
            if let Some(child) = child {
                parent = child;
            } else {
                let next = self.nodes[parent].next;
                self.nodes[index].tree = true;
                self.nodes[index].parent = Some(parent);
                self.nodes[index].prev = Some(parent);
                self.nodes[index].next = next;
                self.nodes[parent].next = Some(index);
                if let Some(next) = next {
                    self.nodes[next].prev = Some(index);
                }
                if left {
                    self.nodes[parent].left = Some(index);
                } else {
                    self.nodes[parent].right = Some(index);
                }
                let root = self.balance(root, index);
                self.move_root(bucket, root);
                return;
            }
        }
    }
    fn resize(&mut self) {
        let old_capacity = self.table.len();
        if old_capacity >= 1 << 30 {
            self.threshold = i32::MAX as usize;
            return;
        }
        let capacity = if old_capacity == 0 {
            16
        } else {
            old_capacity * 2
        };
        self.threshold = capacity * 3 / 4;
        let old = std::mem::replace(&mut self.table, vec![None; capacity]);
        for (bucket, head) in old.into_iter().enumerate() {
            let Some(head) = head else {
                continue;
            };
            if self.nodes[head].next.is_none() {
                self.table[(self.nodes[head].hash as u32 as usize) & (capacity - 1)] = Some(head);
                continue;
            }
            let tree = self.nodes[head].tree;
            let mut low = Vec::new();
            let mut high = Vec::new();
            let mut next = Some(head);
            while let Some(index) = next {
                next = self.nodes[index].next;
                if (self.nodes[index].hash as u32 as usize) & old_capacity == 0 {
                    low.push(index);
                } else {
                    high.push(index);
                }
            }
            let divided = !low.is_empty() && !high.is_empty();
            for (slot, group) in [(bucket, low), (bucket + old_capacity, high)] {
                if group.is_empty() {
                    continue;
                }
                for (position, index) in group.iter().copied().enumerate() {
                    self.nodes[index].next = group.get(position + 1).copied();
                    self.nodes[index].prev = position.checked_sub(1).map(|p| group[p]);
                    if tree && group.len() <= 6 {
                        self.nodes[index].tree = false;
                        self.nodes[index].parent = None;
                        self.nodes[index].left = None;
                        self.nodes[index].right = None;
                        self.nodes[index].red = false;
                    }
                }
                self.table[slot] = group.first().copied();
                if tree && group.len() > 6 && divided {
                    self.treeify(slot);
                }
            }
        }
    }
}
