use super::stopped_launcher_owner::StoppedLauncherOwner;
use super::LauncherStopSigner;
use std::io::{self, BufRead, BufReader, Read};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

pub(super) const INSTANCE_PREFIX: &str = "OWNER_INSTANCE=";
const MAX_REPORT_BYTES: u64 = 4096;

/// 持有原始子进程句柄和专用报告管道的本机 TM 监护器。
/// Rust 持久 Launcher 扩展，对应 Java 无直接类型。
pub struct LauncherProcessMonitor {
    child: Child,
    reports: Mutex<Receiver<io::Result<[u8; 32]>>>,
    owner_instance_id: Option<[u8; 32]>,
    issuer: Arc<()>,
}

impl LauncherProcessMonitor {
    /// 启动受信工作进程并返回监护器。子进程须在 Begin 前报告唯一 TM 实例，业务日志使用 stderr。
    /// 调用方必须控制可执行文件与所有启动参数。释放监护器不会终止该进程。
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let mut child = command.stdout(Stdio::piped()).spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("supervised stdout pipe unavailable"))?;
        let (sender, reports) = mpsc::channel();
        // 管道读取不得阻塞退出检查。身份只能来自这个监护器亲自启动的进程。
        let reader = std::thread::Builder::new()
            .name("seata-tm-owner-report".into())
            .spawn(move || {
                let _ = sender.send(Self::read_instance(stdout));
            });
        if let Err(error) = reader {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok(Self {
            child,
            reports: Mutex::new(reports),
            owner_instance_id: None,
            issuer: Arc::new(()),
        })
    }

    /// 轮询原进程句柄及其身份报告，返回退出证明、尚未确认或读取错误。
    /// 只在操作系统确认进程退出后签发，可多次读取同一证明。
    pub fn try_confirm_exit(&mut self) -> io::Result<Option<StoppedLauncherOwner>> {
        if self.child.try_wait()?.is_none() {
            return Ok(None);
        }
        if self.owner_instance_id.is_none() {
            let reports = self
                .reports
                .get_mut()
                .map_err(|_| io::Error::other("supervised TM report channel unavailable"))?;
            self.owner_instance_id = match reports.try_recv() {
                Ok(result) => Some(result?),
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "supervised TM instance report unavailable",
                    ));
                }
            };
        }
        Ok(self
            .owner_instance_id
            .map(|owner_instance_id| StoppedLauncherOwner {
                owner_instance_id,
                issuer: self.issuer.clone(),
            }))
    }

    /// 在可信控制进程保存的原监护器上，核对证明的签发者及数据库记录的实例 ID。
    pub fn verifies(&self, proof: &StoppedLauncherOwner, owner_instance_id: &[u8; 32]) -> bool {
        Arc::ptr_eq(&self.issuer, &proof.issuer) && proof.owner_instance_id == *owner_instance_id
    }

    /// 只为本监护器已确认退出的实例签署远程挑战；返回可传输签名。
    pub fn sign_exit_proof(
        &self,
        proof: &StoppedLauncherOwner,
        signer: &LauncherStopSigner,
        nonce: &[u8; 32],
    ) -> Option<[u8; 64]> {
        self.verifies(proof, &proof.owner_instance_id)
            .then(|| signer.sign(nonce, &proof.owner_instance_id))
    }

    fn read_instance(stdout: ChildStdout) -> io::Result<[u8; 32]> {
        let mut reader = BufReader::new(stdout.take(MAX_REPORT_BYTES));
        let mut line = String::new();
        while reader.read_line(&mut line)? != 0 {
            if let Some(report) = line.strip_prefix(INSTANCE_PREFIX) {
                let hex = report
                    .strip_suffix('\n')
                    .map(|value| value.strip_suffix('\r').unwrap_or(value))
                    .filter(|value| {
                        value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
                    })
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "invalid TM instance report")
                    })?;
                let mut instance = [0_u8; 32];
                for (index, item) in instance.iter_mut().enumerate() {
                    *item = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                }
                return Ok(instance);
            }
            line.clear();
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing TM instance report",
        ))
    }
}
