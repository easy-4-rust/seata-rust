use seata_protocol::GlobalStatus;
/// 联调专用提交不确定性记录：Finished只表示无记录，不能证明历史提交成功。
/// 对应 Java: DefaultCore#getStatus/#commit的Finished语义，不是生产TM实现。
pub struct UncertainCommit {
    confirmed: bool,
}
impl UncertainCommit {
    /// 返回未获权威提交确认的状态。
    pub const fn new() -> Self {
        Self { confirmed: false }
    }
    /// 参数必须来自已校验Success前缀的原XID查询/同操作应答；只有明确Committed才确认。
    pub fn observe(&mut self, status: GlobalStatus) {
        if status == GlobalStatus::Committed {
            self.confirmed = true;
        }
    }
    /// 返回是否已有明确提交确认，不能单凭Finished提升为true。
    pub const fn confirmed(&self) -> bool {
        self.confirmed
    }
}
#[cfg(test)]
mod tests {
    use super::UncertainCommit;
    use seata_protocol::GlobalStatus;
    #[test]
    fn finished_never_creates_commit_confirmation_and_does_not_erase_prior_authority() {
        let mut state = UncertainCommit::new();
        state.observe(GlobalStatus::Finished);
        assert!(!state.confirmed());
        state.observe(GlobalStatus::Committing);
        assert!(!state.confirmed());
        state.observe(GlobalStatus::Committed);
        assert!(state.confirmed());
        state.observe(GlobalStatus::Finished);
        assert!(state.confirmed());
    }
}
