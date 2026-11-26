use super::LauncherStopVerifier;
use ed25519_dalek::VerifyingKey;
use std::{
    io,
    sync::{Arc, RwLock},
};

/// 独立交给部署控制方的停止签名公钥更新权限。对应 Java 无直接类型。
pub struct LauncherStopKeyRotation(pub(super) Arc<RwLock<Vec<VerifyingKey>>>);

impl LauncherStopKeyRotation {
    /// 原子替换信任清单。接受单把密钥或轮换重叠期的两把不同公钥。
    /// 配置无效时返回错误，现有清单保持原样。
    pub fn replace(&mut self, keys: &[[u8; 32]]) -> io::Result<()> {
        if !matches!(keys, [_] | [_, _]) || keys.len() == 2 && keys[0] == keys[1] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expect one or two distinct TM stop verification keys",
            ));
        }
        let keys: Option<Vec<_>> = keys.iter().map(LauncherStopVerifier::strong_key).collect();
        let keys = keys.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid TM stop verification key",
            )
        })?;
        let mut active = self
            .0
            .write()
            .map_err(|_| io::Error::other("TM stop keyset unavailable"))?;
        *active = keys;
        Ok(())
    }
}
