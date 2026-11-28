use super::UndoCodecError;

/// Undo对象图结构与字节预算；只允许收紧硬上限，非进程heap字节承诺。
/// 对应 Java: JacksonUndoLogParser的Rust资源边界扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndoCodecLimits {
    pub(crate) encoded: usize,
    pub(crate) nodes: usize,
    pub(crate) depth: usize,
    pub(crate) scalar: usize,
    pub(crate) total_scalar: usize,
}
impl UndoCodecLimits {
    /// 参数依次为总编码字节、节点数、嵌套深度、单scalar字节、总scalar字节；返回不超硬上限的预算。
    pub fn new(
        encoded: usize,
        nodes: usize,
        depth: usize,
        scalar: usize,
        total_scalar: usize,
    ) -> Result<Self, UndoCodecError> {
        let maximum = Self::default();
        if encoded == 0
            || encoded > maximum.encoded
            || nodes == 0
            || nodes > maximum.nodes
            || depth == 0
            || depth > maximum.depth
            || scalar == 0
            || scalar > maximum.scalar
            || total_scalar < scalar
            || total_scalar > maximum.total_scalar
        {
            return Err(UndoCodecError::Shape {
                reason: "codec limits",
            });
        }
        Ok(Self {
            encoded,
            nodes,
            depth,
            scalar,
            total_scalar,
        })
    }
}
impl Default for UndoCodecLimits {
    fn default() -> Self {
        Self {
            encoded: 33_554_432,
            nodes: 262_144,
            depth: 32,
            scalar: 1_398_104,
            total_scalar: 25_165_824,
        }
    }
}
