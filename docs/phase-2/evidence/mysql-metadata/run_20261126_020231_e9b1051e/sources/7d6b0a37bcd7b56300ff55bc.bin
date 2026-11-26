use crate::ProtocolError;

/// 正式载荷解析的显式资源预算；这些是本地保护策略，不是Java线缆约束。
/// 对应 Java: SeataSerializerV1 / MergedWarpMessageCodec 的遍历职责；新增Rust资源边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadLimits {
    pub(crate) maximum_payload_length: usize,
    pub(crate) maximum_batch_children: usize,
    pub(crate) maximum_total_nodes: usize,
    pub(crate) maximum_nesting_depth: usize,
}

impl PayloadLimits {
    /// 参数依次为载荷字节数、单批子数、整棵树节点数（含根）、聚合嵌套层数。
    /// 返回已校验预算；深度最多64以约束递归栈，0禁用聚合；单批最多32767符合signed-short。
    pub fn new(
        maximum_payload_length: usize,
        maximum_batch_children: usize,
        maximum_total_nodes: usize,
        maximum_nesting_depth: usize,
    ) -> Result<Self, ProtocolError> {
        if maximum_payload_length < 2
            || maximum_batch_children > i16::MAX as usize
            || maximum_total_nodes == 0
            || maximum_nesting_depth > 64
        {
            return Err(ProtocolError::InvalidConfiguration {
                reason:
                    "payload bytes >= 2, batch <= 32767, nodes >= 1, merge depth <= 64 required",
            });
        }
        Ok(Self {
            maximum_payload_length,
            maximum_batch_children,
            maximum_total_nodes,
            maximum_nesting_depth,
        })
    }
    /// 返回完整载荷字节预算（含两字节类型）。
    pub const fn maximum_payload_length(&self) -> usize {
        self.maximum_payload_length
    }
    /// 返回单个聚合的子消息数量预算。
    pub const fn maximum_batch_children(&self) -> usize {
        self.maximum_batch_children
    }
    /// 返回全树节点数量预算，包含根及每个嵌套容器。
    pub const fn maximum_total_nodes(&self) -> usize {
        self.maximum_total_nodes
    }
    /// 返回允许的聚合嵌套层数；普通消息不增加层数。
    pub const fn maximum_nesting_depth(&self) -> usize {
        self.maximum_nesting_depth
    }
}
