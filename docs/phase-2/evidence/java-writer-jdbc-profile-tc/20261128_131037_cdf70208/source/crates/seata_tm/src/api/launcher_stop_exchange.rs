use super::{
    LauncherProcessMonitor, LauncherStopChallenge, LauncherStopSigner, LauncherStopVerifier,
    StoppedLauncherOwner,
};
use std::{
    io,
    time::{Duration, Instant},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const REQUEST: &[u8; 4] = b"SST1";
const RESPONSE: &[u8; 4] = b"SSR1";

/// 定长的停止证明请求与应答，各连接处理一次。对应 Java 无直接类型。
pub struct LauncherStopExchange;

impl LauncherStopExchange {
    /// 通过调用方提供的认证通道发送挑战，并返回已由预配公钥核验的签名。
    /// 超时上限取传输预算与挑战剩余时间中的较小值。
    pub async fn request(
        stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
        verifier: &LauncherStopVerifier,
        challenge: &LauncherStopChallenge,
        budget: Duration,
    ) -> io::Result<[u8; 64]> {
        Self::check_budget(budget)?;
        if !verifier.accepts_challenge(challenge) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "invalid TM stop challenge",
            ));
        }
        let timeout = budget.min(challenge.deadline.saturating_duration_since(Instant::now()));
        let signature = tokio::time::timeout(timeout, async {
            let mut request = [0; 68];
            request[..4].copy_from_slice(REQUEST);
            request[4..36].copy_from_slice(&challenge.old_instance);
            request[36..].copy_from_slice(challenge.nonce());
            stream.write_all(&request).await?;
            stream.flush().await?;
            Self::read_header(stream, RESPONSE).await?;
            let mut signature = [0; 64];
            stream.read_exact(&mut signature).await?;
            Ok::<_, io::Error>(signature)
        })
        .await
        .map_err(|_| Self::timeout_error())??;
        if !verifier.verifies(
            challenge,
            &signature,
            &challenge.old_instance,
            &challenge.new_instance,
        ) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "invalid TM stop signature",
            ));
        }
        Ok(signature)
    }

    /// 在调用方提供的认证通道上处理一次挑战。仅为请求的已退出实例签名。
    /// 参数为原监护器、其退出证明、原主机签名器及本次读写总预算。
    pub async fn respond(
        stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
        monitor: &LauncherProcessMonitor,
        proof: &StoppedLauncherOwner,
        signer: &LauncherStopSigner,
        budget: Duration,
    ) -> io::Result<()> {
        Self::check_budget(budget)?;
        tokio::time::timeout(budget, async {
            Self::read_header(stream, REQUEST).await?;
            let mut instance = [0; 32];
            let mut nonce = [0; 32];
            stream.read_exact(&mut instance).await?;
            stream.read_exact(&mut nonce).await?;
            if !monitor.verifies(proof, &instance) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "TM stop proof does not match requested instance",
                ));
            }
            let signature = monitor
                .sign_exit_proof(proof, signer, &nonce)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::PermissionDenied, "invalid TM exit proof")
                })?;
            let mut response = [0; 68];
            response[..4].copy_from_slice(RESPONSE);
            response[4..].copy_from_slice(&signature);
            stream.write_all(&response).await?;
            stream.flush().await
        })
        .await
        .map_err(|_| Self::timeout_error())?
    }

    async fn read_header(
        stream: &mut (impl AsyncRead + Unpin),
        expected: &[u8; 4],
    ) -> io::Result<()> {
        let mut header = [0; 4];
        stream.read_exact(&mut header).await?;
        if &header != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid TM stop frame header",
            ));
        }
        Ok(())
    }

    pub(super) fn check_budget(budget: Duration) -> io::Result<()> {
        if budget.is_zero() || budget > Duration::from_secs(60) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid TM stop exchange budget",
            ));
        }
        Ok(())
    }

    fn timeout_error() -> io::Error {
        io::Error::new(io::ErrorKind::TimedOut, "TM stop exchange timed out")
    }
}
