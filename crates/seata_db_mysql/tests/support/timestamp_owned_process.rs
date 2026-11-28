#![allow(clippy::unwrap_used)]
use std::process::{Child, Command, ExitStatus};
/// 仅管理当前夹具实际创建的进程句柄；失败退出时清理，禁止按外部 PID 终止进程。
/// 对应 Java: 无直接对象；Rust 自有故障进程夹具。
pub struct TimestampOwnedProcess {
    child: Child,
}
impl TimestampOwnedProcess {
    /// 参数为自有可执行文件命令；返回实际子进程，Windows 隐藏非交互窗口。
    pub fn spawn(mut command: Command) -> Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        Self {
            child: command.spawn().unwrap(),
        }
    }
    /// 无参数；返回操作系统提供的原子进程 PID。
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
    /// 无参数；返回该实际进程句柄的终态，运行时为 None。
    pub fn status(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().unwrap()
    }
    /// 无参数；返回实际句柄是否仍在运行，不从状态文件猜存活。
    pub fn alive(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }
    /// 无参数；强制终止该自有句柄并等待真正退出，返回实际非成功状态。
    pub fn crash(&mut self) -> ExitStatus {
        assert!(self.alive(), "owned writer exited before requested crash");
        self.child.kill().unwrap();
        let status = self.child.wait().unwrap();
        assert!(!status.success());
        status
    }
}
impl Drop for TimestampOwnedProcess {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) && self.child.kill().is_ok() {
            let _ = self.child.wait();
        }
    }
}
