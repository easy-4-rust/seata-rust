use super::{launcher_stop_signer::stop_message, LauncherStopChallenge, LauncherStopKeyRotation};
use ed25519_dalek::{Signature, VerifyingKey};
use std::{
    io,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

/// 使用预配原主机公钥验证停止证明的接管方。对应 Java 无直接类型。
pub struct LauncherStopVerifier {
    keys: Arc<RwLock<Vec<VerifyingKey>>>,
    scope: [u8; 32],
    identity: Arc<()>,
}

impl LauncherStopVerifier {
    /// 返回部署预配的信任域，供持久接管与来源记录核对。
    pub const fn scope(&self) -> &[u8; 32] {
        &self.scope
    }
    /// 创建固定绑定原主机公钥及部署信任域的验证器。无效公钥无法授权接管。
    pub fn new(key: [u8; 32], scope: [u8; 32]) -> Self {
        Self::new_rotatable(key, scope).0
    }

    /// 创建验证器和独立更新权限。公钥可轮换，信任域与已有挑战的身份保持一致。
    pub fn new_rotatable(key: [u8; 32], scope: [u8; 32]) -> (Self, LauncherStopKeyRotation) {
        let keys = Arc::new(RwLock::new(Self::strong_key(&key).into_iter().collect()));
        let verifier = Self {
            keys: keys.clone(),
            scope,
            identity: Arc::new(()),
        };
        (verifier, LauncherStopKeyRotation(keys))
    }

    /// 为来源实例和申请接管实例生成挑战，期限须大于零且至多 60 秒。
    pub fn challenge(
        &self,
        old_instance: &[u8; 32],
        new_instance: &[u8; 32],
        lifetime: Duration,
    ) -> io::Result<LauncherStopChallenge> {
        if lifetime.is_zero() || lifetime > Duration::from_secs(60) || old_instance == new_instance
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid TM stop challenge",
            ));
        }
        let mut nonce = [0; 32];
        getrandom::getrandom(&mut nonce)
            .map_err(|_| io::Error::other("TM stop challenge random source unavailable"))?;
        Ok(LauncherStopChallenge {
            nonce,
            old_instance: *old_instance,
            new_instance: *new_instance,
            deadline: Instant::now() + lifetime,
            verifier: self.identity.clone(),
        })
    }

    /// 按本验证器发出的新鲜挑战、原实例及接管实例核验签名。
    pub fn verifies(
        &self,
        challenge: &LauncherStopChallenge,
        signature: &[u8; 64],
        old_instance: &[u8; 32],
        new_instance: &[u8; 32],
    ) -> bool {
        if challenge.old_instance != *old_instance || challenge.new_instance != *new_instance {
            return false;
        }
        let Ok(keys) = self.keys.read() else {
            return false;
        };
        let message = stop_message(&self.scope, &challenge.nonce, old_instance);
        let signature = Signature::from_bytes(signature);
        self.accepts_challenge(challenge)
            && keys
                .iter()
                .any(|key| key.verify_strict(&message, &signature).is_ok())
    }

    /// 解析预配的 Ed25519 公钥，拒绝弱公钥。
    pub(super) fn strong_key(bytes: &[u8; 32]) -> Option<VerifyingKey> {
        VerifyingKey::from_bytes(bytes)
            .ok()
            .filter(|key| !key.is_weak())
    }

    pub(super) fn accepts_challenge(&self, challenge: &LauncherStopChallenge) -> bool {
        Arc::ptr_eq(&self.identity, &challenge.verifier) && Instant::now() < challenge.deadline
    }
}
