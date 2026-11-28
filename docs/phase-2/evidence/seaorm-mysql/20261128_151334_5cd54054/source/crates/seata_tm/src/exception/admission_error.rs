use thiserror::Error;

/// 应用作用域准入错误，不表示 TC 对事务作出的决定。
/// 对应 Java: 无直接对象；WP07 应用停机准入扩展。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AdmissionError {
    /// 容量必须在 Tokio 信号量支持的正数范围内。
    #[error("事务准入容量无效")]
    InvalidCapacity,
    /// 当前应用持有的作用域数量已达上限。
    #[error("事务准入容量已满")]
    AtCapacity,
    /// 应用已经开始停机，不再接纳新作用域。
    #[error("事务准入已关闭")]
    Closed,
    /// 期限到达时仍有作用域持有许可；不会取消这些作用域。
    #[error("事务作用域未排空，仍有 {active} 个")]
    DrainTimeout {
        /// 超时观察时尚未释放的许可数量。
        active: usize,
    },
}
