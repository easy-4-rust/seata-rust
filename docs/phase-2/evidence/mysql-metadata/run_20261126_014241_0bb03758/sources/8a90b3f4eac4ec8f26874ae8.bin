use crate::BranchIdentity;
use seata_protocol::transaction::BranchRegisterResponse;
use std::fmt;

/// 正式客户端一次匹配成功响应产生的原始注册回执，无公开构造器或反序列化入口。
/// 对应 Java: AbstractResourceManager#branchRegister 的成功返回；Rust 增加不可伪造的本地交接对象。
/// 回执只证明所配置连接返回成功，不证明数据库提交、全局锁完整性或生产认证。
///
/// ```compile_fail
/// use seata_rm::RegisteredBranch;
/// let forged = RegisteredBranch::new();
/// ```
#[derive(Clone)]
pub struct RegisteredBranch {
    identity: BranchIdentity,
    lock_key: Option<String>,
    response: BranchRegisterResponse,
}
impl RegisteredBranch {
    /// 参数为成功响应核对后的原身份、原锁键和响应；返回仅正式客户端可签发的本地回执。
    pub(crate) fn issued(
        identity: BranchIdentity,
        lock_key: Option<String>,
        response: BranchRegisterResponse,
    ) -> Self {
        Self {
            identity,
            lock_key,
            response,
        }
    }
    /// 无参数；返回 TC 原编号和原申请身份的只读借用，不授权重发注册。
    pub fn identity(&self) -> &BranchIdentity {
        &self.identity
    }
    /// 无参数；返回原申请的可选锁键，不改写或重新生成。
    pub fn lock_key(&self) -> Option<&str> {
        self.lock_key.as_deref()
    }
    /// 无参数；返回原响应前缀及精确 BranchId，保留可选消息与异常码。
    pub fn response(&self) -> &BranchRegisterResponse {
        &self.response
    }
}
impl fmt::Debug for RegisteredBranch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredBranch")
            .field("identity", &self.identity)
            .field("lock_key_bytes", &self.lock_key.as_ref().map(String::len))
            .field("result_code", &self.response.response.result_code)
            .finish_non_exhaustive()
    }
}
