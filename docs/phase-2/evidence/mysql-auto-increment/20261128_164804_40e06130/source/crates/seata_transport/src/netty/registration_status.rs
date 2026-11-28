/// 一个登记步骤的权威/不确定结果；不能把超时解释为identified=false。
/// 对应 Java: AbstractIdentifyResponse#isIdentified与MessageFuture的组合语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationStatus {
    /// 前置步骤失败或尚未开始。
    NotAttempted,
    /// TC有效响应identified=true。
    Accepted,
    /// TC有效响应identified=false。
    Rejected,
    /// 明确未开始此步骤的网络写入。
    LocalFailureBeforeSend,
    /// 步骤可能已生效但没有有效响应。
    UnknownOutcome,
}
