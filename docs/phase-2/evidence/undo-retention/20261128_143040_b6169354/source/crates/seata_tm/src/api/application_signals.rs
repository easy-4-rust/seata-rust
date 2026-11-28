use crate::ShutdownSignalKind;
use std::io;

/// 应用显式启用的 OS 停机事件接收器；仅 application_signals 功能可用。
/// 对应 Java: 无直接对象；WP07 应用停机接入扩展。
///
/// install 应在开启信号与 IO 驱动的 Tokio 运行时内调用。底层处理器是进程级的，
/// Drop 不保证恢复 OS 默认处理方式，连续事件可能合并；第二次事件不会隐式强杀。
/// 不接管 Windows 服务控制事件或系统关机宽限期。应用应先安装监听器，收到事件
/// 后关闭准入并排空，再消耗 TM/RM 所有者；排空超时仍须保留所有者及原事务身份。
///
/// 应用在接收业务之前 install，停机路径保留所有者直到排空成功：
///
///     use seata_tm::{ApplicationSignals, ShutdownSignalKind, TransactionAdmission, TransactionManager};
///     use std::{error::Error, time::Duration};
///
///     async fn stop(
///         signals: &mut ApplicationSignals,
///         gate: &TransactionAdmission,
///         owner: &mut Option<TransactionManager>,
///     ) -> Result<ShutdownSignalKind, Box<dyn Error>> {
///         let reason = signals.recv().await?;
///         gate.drain(Duration::from_secs(30)).await?;
///         if let Some(manager) = owner.take() {
///             manager.shutdown().await?;
///         }
///         Ok(reason)
///     }
#[derive(Debug)]
pub struct ApplicationSignals {
    #[cfg(windows)]
    interrupt: tokio::signal::windows::CtrlC,
    #[cfg(windows)]
    console_break: tokio::signal::windows::CtrlBreak,
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
}

impl ApplicationSignals {
    /// 安装当前平台监听器；无参数，返回接收器或 OS 注册错误。
    /// Windows 监听 Ctrl-C/Ctrl-Break，Unix 监听 SIGINT/SIGTERM。
    /// 错误也可能发生于部分处理器已注册之后；调用必须处于 Tokio 运行时内。
    pub fn install() -> io::Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self {
                interrupt: tokio::signal::windows::ctrl_c()?,
                console_break: tokio::signal::windows::ctrl_break()?,
            })
        }
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            Ok(Self {
                interrupt: signal(SignalKind::interrupt())?,
                terminate: signal(SignalKind::terminate())?,
            })
        }
    }

    /// 等待并返回下一种事件；无参数，接收器关闭时返回 BrokenPipe。
    /// 取消此次等待后可以继续使用同一接收器；不会更改准入或事务状态。
    pub async fn recv(&mut self) -> io::Result<ShutdownSignalKind> {
        #[cfg(windows)]
        let (event, kind) = tokio::select! {
            event = self.interrupt.recv() => (event, ShutdownSignalKind::Interrupt),
            event = self.console_break.recv() => (event, ShutdownSignalKind::ConsoleBreak),
        };
        #[cfg(unix)]
        let (event, kind) = tokio::select! {
            event = self.interrupt.recv() => (event, ShutdownSignalKind::Interrupt),
            event = self.terminate.recv() => (event, ShutdownSignalKind::Terminate),
        };
        event.map(|()| kind).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "application signal receiver closed",
            )
        })
    }
}
