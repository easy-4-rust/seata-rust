use crate::DbError;

/// 注册字段的知识来源；回调来源明确表示没有原锁键信息，不能与已知 None 混淆。
/// 对应 Java: 无直接对象；Rust 对 BranchRegisterResponse 与二阶段请求的来源区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistrationOrigin {
    Registered,
    Callback,
}
impl RegistrationOrigin {
    /// 参数为扩展表的原来源字节；返回已知来源，未知编号拒绝。
    pub(crate) fn from_stored(value: u8) -> Result<Self, DbError> {
        match value {
            1 => Ok(Self::Registered),
            2 => Ok(Self::Callback),
            _ => Err(DbError::Unsupported {
                feature: "RM recovery registration origin",
            }),
        }
    }
}
