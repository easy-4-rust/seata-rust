//! 故障联调专用的内存分支动作，不是生产RM/Undo或持久幂等实现。
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchCommitResponse, BranchRollbackResponse,
};
use seata_protocol::{BranchStatus, BranchType, Message, ResultCode, TransactionExceptionCode};
use std::error::Error;
use std::io;
/// 用客户端发送前已知的身份校验TC回调，只有匹配的取消回调才可学习丢失的BranchId。
/// 对应 Java: BranchCommitRequest/BranchRollbackRequest的身份关联职责；仅测试模拟资源。
pub struct SimulatedBranch {
    xid: String,
    resource: String,
    data: String,
    commit: bool,
    branch_id: Option<i64>,
    cached: Option<Message>,
    actions: u32,
}
impl SimulatedBranch {
    /// 参数为原事务/资源/业务nonce、预期阶段和已知分支号；返回尚未执行动作的模拟状态。
    pub fn new(
        xid: String,
        resource: String,
        data: String,
        commit: bool,
        branch_id: Option<i64>,
    ) -> Self {
        Self {
            xid,
            resource,
            data,
            commit,
            branch_id,
            cached: None,
            actions: 0,
        }
    }
    /// 参数为TC实际回调消息；校验全部已知身份后返回原身份结果，只首次产生内存动作。
    pub fn prepare(&mut self, message: &Message) -> Result<Message, Box<dyn Error>> {
        let (commit, xid, branch_id, branch_type, resource, data) = match message {
            Message::BranchCommitRequest(value) => (
                true,
                &value.xid,
                value.branch_id,
                value.branch_type,
                &value.resource_id,
                value.application_data.as_deref(),
            ),
            Message::BranchRollbackRequest(value) => (
                false,
                &value.xid,
                value.branch_id,
                value.branch_type,
                &value.resource_id,
                value.application_data.as_deref(),
            ),
            _ => return Err(io::Error::other("not a branch-end callback").into()),
        };
        if self.data.is_empty()
            || commit != self.commit
            || xid != &self.xid
            || resource != &self.resource
            || data != Some(self.data.as_str())
            || branch_type != BranchType::At
            || branch_id <= 0
            || self.branch_id.is_some_and(|known| known != branch_id)
            || (self.branch_id.is_none() && commit)
        {
            return Err(io::Error::other(
                "callback does not match original pending branch identity",
            )
            .into());
        }
        self.branch_id = Some(branch_id);
        if let Some(cached) = &self.cached {
            return Ok(cached.clone());
        }
        let response = AbstractTransactionResponse {
            result_code: ResultCode::Success,
            msg: None,
            transaction_exception_code: TransactionExceptionCode::Unknown,
        };
        let message = if commit {
            Message::BranchCommitResponse(BranchCommitResponse {
                response,
                xid: Some(self.xid.clone()),
                branch_id,
                branch_status: BranchStatus::PhaseTwoCommitted,
            })
        } else {
            Message::BranchRollbackResponse(BranchRollbackResponse {
                response,
                xid: Some(self.xid.clone()),
                branch_id,
                branch_status: BranchStatus::PhaseTwoRollbacked,
            })
        };
        self.actions += 1;
        self.cached = Some(message.clone());
        Ok(message)
    }
    /// 返回已校验/学习的分支号；未匹配回调前保持None。
    pub const fn branch_id(&self) -> Option<i64> {
        self.branch_id
    }
    /// 返回内存动作数；不等同实际SQL/Undo补偿次数。
    pub const fn actions(&self) -> u32 {
        self.actions
    }
}
#[cfg(test)]
mod tests {
    use super::SimulatedBranch;
    use seata_protocol::transaction::BranchRollbackRequest;
    use seata_protocol::{BranchType, Message};
    fn request() -> BranchRollbackRequest {
        BranchRollbackRequest {
            xid: "x".into(),
            branch_id: 42,
            branch_type: BranchType::At,
            resource_id: "r".into(),
            application_data: Some("nonce".into()),
        }
    }
    #[test]
    fn pending_identity_is_learned_only_from_exact_callback_and_duplicate_action_is_cached() {
        let mut state = SimulatedBranch::new("x".into(), "r".into(), "nonce".into(), false, None);
        let first = state
            .prepare(&Message::BranchRollbackRequest(request()))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            state
                .prepare(&Message::BranchRollbackRequest(request()))
                .unwrap_or_else(|error| panic!("{error}")),
            first
        );
        assert_eq!(state.branch_id(), Some(42));
        assert_eq!(state.actions(), 1);
    }
    #[test]
    fn mismatches_neither_learn_branch_nor_execute_simulated_action() {
        for field in 0..5 {
            let mut state =
                SimulatedBranch::new("x".into(), "r".into(), "nonce".into(), false, None);
            let mut value = request();
            match field {
                0 => value.xid = "other".into(),
                1 => value.resource_id = "other".into(),
                2 => value.application_data = Some("other".into()),
                3 => value.branch_type = BranchType::Xa,
                _ => value.branch_id = 0,
            };
            assert!(state
                .prepare(&Message::BranchRollbackRequest(value))
                .is_err());
            assert_eq!(state.branch_id(), None);
            assert_eq!(state.actions(), 0);
        }
    }
}
