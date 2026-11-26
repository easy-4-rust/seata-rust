use std::{sync::Arc, time::Instant};

/// 接管方为旧实例和申请接管的新实例生成的限时挑战。对应 Java 无直接类型。
pub struct LauncherStopChallenge {
    pub(super) nonce: [u8; 32],
    pub(super) old_instance: [u8; 32],
    pub(super) new_instance: [u8; 32],
    pub(super) deadline: Instant,
    pub(super) verifier: Arc<()>,
}

impl LauncherStopChallenge {
    /// 返回发送给原主机监护器的 32 字节随机挑战。
    pub const fn nonce(&self) -> &[u8; 32] {
        &self.nonce
    }
}
