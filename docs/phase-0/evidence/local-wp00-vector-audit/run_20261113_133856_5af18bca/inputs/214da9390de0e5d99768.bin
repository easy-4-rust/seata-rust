use crate::branch_end_request::BranchEndRequest;
use std::io;

/// 单个模拟分支的进程内回调状态；不是持久化幂等或业务原子性实现。
/// 对应 Java: AbstractRMHandler#doBranchCommit / doBranchRollback 的探针验证状态，无生产对象移植。
pub(super) struct BranchCallbackState {
    kind: u16,
    xid: String,
    branch_id: i64,
    resource: String,
    completed: bool,
}

impl BranchCallbackState {
    /// 根据预期操作、XID、分支 ID 与资源构建状态，返回仅供该分支使用的缓存。
    pub(super) fn new(kind: u16, xid: &str, branch_id: i64, resource: &str) -> Self {
        Self {
            kind,
            xid: xid.to_owned(),
            branch_id,
            resource: resource.to_owned(),
            completed: false,
        }
    }

    /// 校验请求身份，首次成功执行 apply 后缓存结果；返回使用当前请求 ID 的响应字节。
    /// apply 失败时返回错误且不标记完成；缓存仅存活于当前进程，不能代表真实数据库操作。
    pub(super) fn reply(
        &mut self,
        request: &BranchEndRequest,
        apply: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        if !matches!(self.kind, 3 | 5)
            || request.kind != self.kind
            || request.xid != self.xid
            || request.branch_id != self.branch_id
            || request.branch_type != 0
            || request.resource != self.resource
            || request.application_data != "{}"
        {
            return Err(crate::invalid(format!(
                "callback identity mismatch: xid={} branchId={} type={} resource={}",
                request.xid, request.branch_id, request.kind, request.resource
            )));
        }
        let mut body = vec![1, 0];
        crate::write_string16(&self.xid, &mut body)?;
        body.extend_from_slice(&self.branch_id.to_be_bytes());
        body.push(if self.kind == 3 { 5 } else { 8 });
        let mut frame = crate::build_v1_request(self.kind + 1, request.request_id, &body)?;
        frame[9] = crate::RESPONSE;
        // 只缓存成功的模拟动作；每次投递重新关联 RPC ID，不能重用旧帧头。
        if !self.completed {
            apply()?;
            self.completed = true;
        }
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::BranchCallbackState;
    use crate::branch_end_request::BranchEndRequest;
    fn request(kind: u16, id: u32) -> BranchEndRequest {
        BranchEndRequest {
            request_id: id,
            kind,
            xid: "x".to_owned(),
            branch_id: 42,
            branch_type: 0,
            resource: "r".to_owned(),
            application_data: "{}".to_owned(),
        }
    }
    fn hex(value: &str) -> Vec<u8> {
        (0..value.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&value[at..at + 2], 16).unwrap())
            .collect()
    }
    #[test]
    fn duplicate_commit_replies_to_new_rpc_id_without_reapplying_action() {
        let mut state = BranchCallbackState::new(3, "x", 42, "r");
        let mut actions = 0;
        let first = state
            .reply(&request(3, 77), || {
                actions += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            first,
            hex("dada010000002000100101000000004d00040100000178000000000000002a05")
        );
        let repeated = state
            .reply(&request(3, 78), || {
                actions += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            repeated,
            hex("dada010000002000100101000000004e00040100000178000000000000002a05")
        );
        assert_eq!(actions, 1);
    }
    #[test]
    fn duplicate_rollback_preserves_terminal_outcome_for_same_or_new_rpc_id() {
        let mut state = BranchCallbackState::new(5, "x", 42, "r");
        let mut actions = 0;
        for id in [77, 77, 78] {
            let reply = state
                .reply(&request(5, id), || {
                    actions += 1;
                    Ok(())
                })
                .unwrap();
            assert_eq!(&reply[16..], &hex("00060100000178000000000000002a08"));
            assert_eq!(&reply[12..16], &id.to_be_bytes());
        }
        assert_eq!(actions, 1);
    }
    #[test]
    fn conflicting_redelivery_is_rejected_before_applying_or_returning_cached_success() {
        let mut state = BranchCallbackState::new(3, "x", 42, "r");
        state.reply(&request(3, 77), || Ok(())).unwrap();
        for field in 0..6 {
            let mut retry = request(3, 78);
            match field {
                0 => retry.kind = 5,
                1 => retry.xid = "other".to_owned(),
                2 => retry.branch_id = 43,
                3 => retry.resource = "other".to_owned(),
                4 => retry.application_data = "changed".to_owned(),
                _ => retry.branch_type = 1,
            }
            let mut applied = false;
            assert!(state
                .reply(&retry, || {
                    applied = true;
                    Ok(())
                })
                .is_err());
            assert!(!applied);
        }
    }
    #[test]
    fn failed_simulated_action_is_not_cached_as_success() {
        let mut state = BranchCallbackState::new(3, "x", 42, "r");
        assert!(state
            .reply(&request(3, 77), || Err(crate::invalid(
                "simulated action failed"
            )))
            .is_err());
        let mut applied = false;
        state
            .reply(&request(3, 78), || {
                applied = true;
                Ok(())
            })
            .unwrap();
        assert!(applied);
    }
}
