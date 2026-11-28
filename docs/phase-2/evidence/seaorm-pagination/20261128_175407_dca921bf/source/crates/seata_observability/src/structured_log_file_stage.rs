/// 固定文件失败阶段，不包含调用方路径或原错误文本。
/// 对应 Java: 无直接对象；WP07 Rust 文件输出阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredLogFileStage {
    /// 文件元数据查询。
    Metadata,
    /// 原文件当前位置查询。
    Position,
    /// 原 JSON 完整写入。
    Write,
    /// 等待底层写入完成。
    Flush,
    /// 原文件 sync_all。
    Sync,
}
