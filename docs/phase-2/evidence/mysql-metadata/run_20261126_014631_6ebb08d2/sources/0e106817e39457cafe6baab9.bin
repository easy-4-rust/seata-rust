use crate::DbError;

/// 按原 MySQL Undo 分片规则规划有序子片及根片；完整上下文形成前不发布写入行。
/// 对应 Java: MySQLUndoLogManager#insertUndoLogWithNormal 的分片、subId 和父号布局。
pub(super) struct UndoFragmentLayout<'a> {
    branch_id: i64,
    bytes: &'a [u8],
    limit: usize,
    child_ids: Vec<i64>,
    child_context: String,
    root_context: String,
}

impl<'a> UndoFragmentLayout<'a> {
    pub(super) const MAX_BYTES: usize = 33_554_432;
    const MAX_CONTEXT: usize = 128;
    const SUB_IDS_PREFIX: &'static str = "&subId=";

    pub(super) fn new(branch_id: i64, packet: u64, bytes: &'a [u8]) -> Result<Self, DbError> {
        if branch_id <= 0 || bytes.is_empty() || bytes.len() > Self::MAX_BYTES {
            return Err(Self::unsupported(
                "Undo root identity or payload byte budget",
            ));
        }
        // 生产调用方先核对服务器变量的范围，原 Java 小载荷也可使用同一分片算法。
        if !(2..=1_073_741_824).contains(&packet) {
            return Err(Self::unsupported("Undo max_allowed_packet"));
        }
        let limit = (packet * 4 / 5) as usize;
        let root_context = format!("serializer=jackson&compressorType=NONE&map={packet}");
        let child_count = bytes.len().div_ceil(limit) - 1;
        // 每个子号最少占一位数字和一个分隔符，先排除任何身份都无法表示的布局。
        if child_count > 0
            && root_context.len() + Self::SUB_IDS_PREFIX.len() + child_count * 2 > Self::MAX_CONTEXT
        {
            return Err(Self::unsupported("Undo subId context capacity"));
        }
        Ok(Self {
            branch_id,
            bytes,
            limit,
            child_ids: Vec::with_capacity(child_count),
            child_context: format!("branchId={branch_id}"),
            root_context,
        })
    }

    pub(super) fn child_count(&self) -> usize {
        self.bytes.len().div_ceil(self.limit) - 1
    }

    pub(super) fn add_child(&mut self, id: i64) -> Result<(), DbError> {
        if id <= 0 || id == self.branch_id || self.child_ids.contains(&id) {
            return Err(Self::unsupported("Undo subId collision"));
        }
        if self.child_ids.len() == self.child_count() {
            return Err(Self::unsupported("Undo unexpected child identity"));
        }
        let prefix = if self.child_ids.is_empty() {
            Self::SUB_IDS_PREFIX
        } else {
            ""
        };
        let text = id.to_string();
        if self.root_context.len() + prefix.len() + text.len() + 1 > Self::MAX_CONTEXT {
            return Err(Self::unsupported("Undo subId context capacity"));
        }
        self.root_context.push_str(prefix);
        self.root_context.push_str(&text);
        self.root_context.push(',');
        self.child_ids.push(id);
        Ok(())
    }

    pub(super) fn rows(&self) -> Result<impl Iterator<Item = (i64, &str, &[u8])> + '_, DbError> {
        if self.child_ids.len() != self.child_count() {
            return Err(Self::unsupported("Undo missing child identities"));
        }
        let children = self
            .child_ids
            .iter()
            .copied()
            .zip(self.bytes.chunks(self.limit).skip(1))
            .map(|(id, chunk)| (id, self.child_context.as_str(), chunk));
        let root = (
            self.branch_id,
            self.root_context.as_str(),
            &self.bytes[..self.bytes.len().min(self.limit)],
        );
        Ok(children.chain(std::iter::once(root)))
    }

    fn unsupported(feature: &'static str) -> DbError {
        DbError::Unsupported { feature }
    }
}
