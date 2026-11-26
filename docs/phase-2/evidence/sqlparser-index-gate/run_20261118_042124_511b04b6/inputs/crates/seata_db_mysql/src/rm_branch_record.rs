use crate::{registration_origin::RegistrationOrigin, DbError};

/// 锁定当前读取后校验的恢复状态；来源与方向分离，不依赖 NULL 猜测知识状态。
/// 对应 Java: 无直接对象；WP05 Rust 版本化持久恢复记录。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RmBranchRecord {
    /// 原绑定方向：0 未绑定、1 提交、2 回滚。
    pub(crate) phase: u8,
    /// 本方向效果是否已经原子持久提交；两种物理终态仍分开存储。
    pub(crate) terminal: bool,
    /// 正式提交回调是否已经持久确认，不从原登记来源或旧方向推断。
    pub(crate) commit_authorized: bool,
    /// 原锁键字段是否来自真正的成功注册申请。
    pub(crate) origin: RegistrationOrigin,
}
impl RmBranchRecord {
    /// 参数为持久方向、终态和来源；返回有效状态，回调来源不能留下未绑定方向。
    pub(crate) fn checked(
        phase: u8,
        terminal: u8,
        commit_authorized: u8,
        commit_terminal: u8,
        origin: RegistrationOrigin,
    ) -> Result<Self, DbError> {
        if phase > 2
            || terminal > 1
            || commit_authorized > 1
            || commit_terminal > 1
            || (terminal == 1 && phase != 2)
            || (commit_authorized == 1 && phase != 1)
            || (commit_terminal == 1 && (phase != 1 || commit_authorized != 1))
            || (origin == RegistrationOrigin::Callback && phase == 0)
        {
            return Err(DbError::Unsupported {
                feature: "RM recovery legal phase, terminal and origin",
            });
        }
        Ok(Self {
            phase,
            terminal: terminal == 1 || commit_terminal == 1,
            commit_authorized: commit_authorized == 1,
            origin,
        })
    }
}
