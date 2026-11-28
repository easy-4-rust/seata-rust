use crate::TransportError;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// 进程内不重复的连接代际，和线缆上的32位请求ID分开；不依赖时钟或端口复用。
/// 对应 Java: io.netty.channel.ChannelId 的通道身份职责；Rust使用checked进程代际。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionGeneration(u64);
impl ConnectionGeneration {
    /// 返回代际数值，仅作通道身份/观测，不写入V1帧。
    pub const fn value(self) -> u64 {
        self.0
    }
    pub(crate) fn next() -> Result<Self, TransportError> {
        Self::from_counter(&NEXT_GENERATION)
    }
    fn from_counter(counter: &AtomicU64) -> Result<Self, TransportError> {
        let mut current = counter.load(Ordering::Relaxed);
        loop {
            let next = current
                .checked_add(1)
                .ok_or(TransportError::InvalidConfiguration {
                    reason: "connection generation space exhausted",
                })?;
            // compare_exchange兼容MSRV1.85，且不依赖新版本已重命名的fetch_update。
            match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(value) => return Ok(Self(value)),
                Err(value) => current = value,
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::ConnectionGeneration;
    use std::sync::atomic::AtomicU64;
    #[test]
    fn generation_counter_never_wraps_or_reuses_zero() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(
            ConnectionGeneration::from_counter(&counter)
                .ok()
                .map(ConnectionGeneration::value),
            Some(u64::MAX - 1)
        );
        assert!(ConnectionGeneration::from_counter(&counter).is_err());
        assert!(ConnectionGeneration::from_counter(&counter).is_err());
    }
}
