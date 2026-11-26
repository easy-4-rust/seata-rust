use crate::LockKeyError;
use std::cmp::Ordering;

/// 只复现原PRIMARY列插入后Java17 HashMap的迭代次序，不替代SQL主键索引序或通用Map。
/// 对应 Java: TableMeta#getPrimaryKeyOnlyName/getPrimaryKeyMap与java.util.HashMap的有界插入行为。
pub(crate) struct JavaPrimaryKeyOrder<'a> {
    names: &'a [&'a str],
    nodes: Vec<OrderNode>,
    buckets: Vec<OrderBucket>,
}
struct OrderNode {
    hash: i32,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
    red: bool,
}
#[derive(Default)]
struct OrderBucket {
    chain: Vec<usize>,
    root: Option<usize>,
}
impl<'a> JavaPrimaryKeyOrder<'a> {
    pub(crate) fn indices(names: &'a [&'a str]) -> Result<Vec<usize>, LockKeyError> {
        if names.is_empty() || names.len() > 128 || names.iter().any(|name| !name.is_ascii()) {
            return Err(Self::invalid());
        }
        let mut map = Self {
            names,
            nodes: Vec::with_capacity(names.len()),
            buckets: Vec::new(),
        };
        map.resize()?;
        for (index, name) in names.iter().enumerate() {
            let hash = name.bytes().fold(0_u32, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(u32::from(byte))
            });
            map.nodes.push(OrderNode {
                hash: (hash ^ (hash >> 16)) as i32,
                parent: None,
                left: None,
                right: None,
                red: false,
            });
            let bucket = map.nodes[index].hash as u32 as usize & (map.buckets.len() - 1);
            if map.buckets[bucket].root.is_some() {
                map.insert_tree(bucket, index, true)?;
            } else {
                let previous_length = map.buckets[bucket].chain.len();
                map.buckets[bucket].chain.push(index);
                // 原putVal的binCount从0开始；第9个同桶节点才触发treeifyBin。
                if previous_length >= 8 {
                    if map.buckets.len() < 64 {
                        map.resize()?;
                    } else {
                        map.treeify(bucket)?;
                    }
                }
            }
            if map.nodes.len() > map.buckets.len() * 3 / 4 {
                map.resize()?;
            }
        }
        Ok(map
            .buckets
            .into_iter()
            .flat_map(|bucket| bucket.chain)
            .collect())
    }
    fn invalid() -> LockKeyError {
        LockKeyError::Invalid {
            reason: "Java primary key ordering invariant",
        }
    }
    fn compare(&self, first: usize, second: usize) -> Ordering {
        self.nodes[first]
            .hash
            .cmp(&self.nodes[second].hash)
            .then_with(|| self.names[first].cmp(self.names[second]))
    }
    fn resize(&mut self) -> Result<(), LockKeyError> {
        let old_capacity = self.buckets.len();
        let new_capacity = if old_capacity == 0 {
            16
        } else {
            old_capacity * 2
        };
        if new_capacity > 256 {
            return Err(Self::invalid());
        }
        let old = std::mem::take(&mut self.buckets);
        self.buckets = (0..new_capacity).map(|_| OrderBucket::default()).collect();
        for (index, bucket) in old.into_iter().enumerate() {
            let was_tree = bucket.root.is_some();
            let mut low = OrderBucket::default();
            let mut high = OrderBucket::default();
            for node in bucket.chain {
                if self.nodes[node].hash as u32 as usize & old_capacity == 0 {
                    low.chain.push(node);
                } else {
                    high.chain.push(node);
                }
            }
            let split = !low.chain.is_empty() && !high.chain.is_empty();
            for (target, mut next) in [(index, low), (index + old_capacity, high)] {
                if was_tree && next.chain.len() > 6 {
                    next.root = bucket.root;
                }
                self.buckets[target] = next;
                if was_tree && self.buckets[target].chain.len() > 6 && split {
                    self.treeify(target)?;
                }
            }
        }
        Ok(())
    }
    fn treeify(&mut self, bucket: usize) -> Result<(), LockKeyError> {
        self.buckets[bucket].root = None;
        let chain = self.buckets[bucket].chain.clone();
        for node in chain {
            self.nodes[node].parent = None;
            self.nodes[node].left = None;
            self.nodes[node].right = None;
            self.nodes[node].red = false;
            self.insert_tree(bucket, node, false)?;
        }
        self.front(bucket)
    }
    fn insert_tree(&mut self, bucket: usize, node: usize, link: bool) -> Result<(), LockKeyError> {
        let Some(mut parent) = self.buckets[bucket].root else {
            self.buckets[bucket].root = Some(node);
            self.nodes[node].red = false;
            return Ok(());
        };
        for _ in 0..=self.nodes.len() {
            let left = self.compare(node, parent) == Ordering::Less;
            let child = if left {
                self.nodes[parent].left
            } else {
                self.nodes[parent].right
            };
            if let Some(child) = child {
                parent = child;
                continue;
            }
            if left {
                self.nodes[parent].left = Some(node);
            } else {
                self.nodes[parent].right = Some(node);
            }
            self.nodes[node].parent = Some(parent);
            if link {
                let position = self.buckets[bucket]
                    .chain
                    .iter()
                    .position(|entry| *entry == parent)
                    .ok_or(Self::invalid())?;
                self.buckets[bucket].chain.insert(position + 1, node);
            }
            let root = self.buckets[bucket].root.ok_or(Self::invalid())?;
            self.buckets[bucket].root = Some(self.balance(root, node)?);
            if link {
                self.front(bucket)?;
            }
            return Ok(());
        }
        Err(Self::invalid())
    }
    fn front(&mut self, bucket: usize) -> Result<(), LockKeyError> {
        let root = self.buckets[bucket].root.ok_or(Self::invalid())?;
        let position = self.buckets[bucket]
            .chain
            .iter()
            .position(|node| *node == root)
            .ok_or(Self::invalid())?;
        self.buckets[bucket].chain.remove(position);
        self.buckets[bucket].chain.insert(0, root);
        Ok(())
    }
    fn rotate(&mut self, root: &mut usize, pivot: usize, left: bool) -> Result<(), LockKeyError> {
        let child = (if left {
            self.nodes[pivot].right
        } else {
            self.nodes[pivot].left
        })
        .ok_or(Self::invalid())?;
        let middle = if left {
            self.nodes[child].left
        } else {
            self.nodes[child].right
        };
        if left {
            self.nodes[pivot].right = middle;
        } else {
            self.nodes[pivot].left = middle;
        }
        if let Some(middle) = middle {
            self.nodes[middle].parent = Some(pivot);
        }
        let parent = self.nodes[pivot].parent;
        self.nodes[child].parent = parent;
        if let Some(parent) = parent {
            if self.nodes[parent].left == Some(pivot) {
                self.nodes[parent].left = Some(child);
            } else {
                self.nodes[parent].right = Some(child);
            }
        } else {
            *root = child;
        }
        if left {
            self.nodes[child].left = Some(pivot);
        } else {
            self.nodes[child].right = Some(pivot);
        }
        self.nodes[pivot].parent = Some(child);
        Ok(())
    }
    fn balance(&mut self, mut root: usize, mut node: usize) -> Result<usize, LockKeyError> {
        self.nodes[node].red = true;
        for _ in 0..=self.nodes.len() {
            let Some(parent) = self.nodes[node].parent else {
                self.nodes[node].red = false;
                return Ok(node);
            };
            if !self.nodes[parent].red {
                return Ok(root);
            }
            let Some(grand) = self.nodes[parent].parent else {
                return Ok(root);
            };
            let parent_left = self.nodes[grand].left == Some(parent);
            let uncle = if parent_left {
                self.nodes[grand].right
            } else {
                self.nodes[grand].left
            };
            if uncle.is_some_and(|uncle| self.nodes[uncle].red) {
                if let Some(uncle) = uncle {
                    self.nodes[uncle].red = false;
                }
                self.nodes[parent].red = false;
                self.nodes[grand].red = true;
                node = grand;
                continue;
            }
            if (if parent_left {
                self.nodes[parent].right
            } else {
                self.nodes[parent].left
            }) == Some(node)
            {
                node = parent;
                self.rotate(&mut root, node, parent_left)?;
            }
            let parent = self.nodes[node].parent.ok_or(Self::invalid())?;
            self.nodes[parent].red = false;
            if let Some(grand) = self.nodes[parent].parent {
                self.nodes[grand].red = true;
                self.rotate(&mut root, grand, !parent_left)?;
            }
        }
        Err(Self::invalid())
    }
}
