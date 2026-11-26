use ring::signature::{Ed25519KeyPair, KeyPair};
use std::io;

/// 供原主机可信监护器持有的停止证明签名密钥。对应 Java 无直接类型。
pub struct LauncherStopSigner {
    key: Ed25519KeyPair,
    scope: [u8; 32],
}

impl LauncherStopSigner {
    /// 加载 Ed25519 PKCS#8 私钥，并绑定部署分配的 32 字节信任域。
    pub fn from_pkcs8(key: &[u8], scope: [u8; 32]) -> io::Result<Self> {
        let key = Ed25519KeyPair::from_pkcs8(key).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid Ed25519 PKCS#8 key")
        })?;
        Ok(Self { key, scope })
    }

    /// 返回应通过部署可信渠道预配给接管方的 Ed25519 公钥。
    pub fn public_key(&self) -> [u8; 32] {
        let mut public_key = [0; 32];
        public_key.copy_from_slice(self.key.public_key().as_ref());
        public_key
    }

    pub(super) fn sign(&self, nonce: &[u8; 32], owner: &[u8; 32]) -> [u8; 64] {
        let signature = self.key.sign(&stop_message(&self.scope, nonce, owner));
        let mut bytes = [0; 64];
        bytes.copy_from_slice(signature.as_ref());
        bytes
    }
}

pub(super) fn stop_message(scope: &[u8; 32], nonce: &[u8; 32], owner: &[u8; 32]) -> Vec<u8> {
    // 固定版本与固定宽度字段避免不同用途或拼接边界产生相同签名输入。
    [
        b"seata-rust/tm/owner-stopped/v1\0".as_slice(),
        scope,
        nonce,
        owner,
    ]
    .concat()
}
