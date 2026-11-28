//! 有界自有恢复子进程；始终隐藏窗口并保留实际输出。
use std::{path::PathBuf, time::Duration};
pub(super) async fn child(
    executable: PathBuf,
    journal: PathBuf,
    out: PathBuf,
) -> std::process::Output {
    tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new(executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut process = command
            .env("SEATA_RM_CHILD_JOURNAL", journal)
            .env("SEATA_RM_CHILD_OUT", out)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(20);
        while process.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= until {
                process.kill().unwrap();
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        process.wait_with_output().unwrap()
    })
    .await
    .unwrap()
}
