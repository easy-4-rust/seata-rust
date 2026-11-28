/// 应用显式监听到的停机事件，不是 TC 对事务作出的决定。
/// 对应 Java: 无直接对象；WP07 应用停机接入扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShutdownSignalKind {
    /// Windows Ctrl-C 或 Unix SIGINT。
    Interrupt,
    /// Unix SIGTERM；Windows 控制台监听器不生成此值。
    Terminate,
    /// Windows Ctrl-Break；Unix 监听器不生成此值。
    ConsoleBreak,
}
