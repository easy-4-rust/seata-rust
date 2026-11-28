use std::sync::Arc;

/// 恢复结束发送的管理器权限，交给部署中的受信恢复控制方。
/// 对应 Java 无直接类型。
pub struct RecoveredEndAuthority(pub(super) Arc<()>);
