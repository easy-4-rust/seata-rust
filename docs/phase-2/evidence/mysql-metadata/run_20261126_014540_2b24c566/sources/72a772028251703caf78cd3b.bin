use std::sync::Arc;

/// 监护器确认同一子进程已退出后签发的本机证明。
/// Rust 持久 Launcher 扩展，对应 Java 无直接类型。
pub struct StoppedLauncherOwner {
    pub(super) owner_instance_id: [u8; 32],
    pub(super) issuer: Arc<()>,
}
