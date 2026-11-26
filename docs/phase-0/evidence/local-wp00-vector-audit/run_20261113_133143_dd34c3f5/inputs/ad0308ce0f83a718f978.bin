use crate::branch_end_request::BranchEndRequest;
use std::io;

/// 应答丢失后仅凭合法回调学习分支 ID 的探针状态；每个上下文只有一个待确认注册。
/// 对应 Java: BranchRollbackRequest 的身份与 applicationData 关联职责；不计作生产移植。
pub(super) struct PendingBranch {
    xid: String,
    resource: String,
    application_data: String,
    pub(super) learned_branch_id: Option<i64>,
    completed: bool,
}

impl PendingBranch {
    /// 输入发送前已知的 XID、资源与唯一 applicationData 标记，返回尚无 BranchId 的状态。
    pub(super) fn new(xid: &str, resource: &str, application_data: &str) -> Self {
        Self {
            xid: xid.into(),
            resource: resource.into(),
            application_data: application_data.into(),
            learned_branch_id: None,
            completed: false,
        }
    }
    /// 校验取消回调后学习正数 BranchId，首次成功 apply 后缓存；返回当前 RPC ID 的响应或错误。
    pub(super) fn reply(
        &mut self,
        request: &BranchEndRequest,
        apply: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        if self.application_data.is_empty()
            || request.application_data != self.application_data
            || request.kind != 5
            || request.xid != self.xid
            || request.resource != self.resource
            || request.branch_type != 0
            || request.branch_id <= 0
            || self
                .learned_branch_id
                .is_some_and(|id| id != request.branch_id)
        {
            return Err(crate::invalid("pending branch callback identity mismatch"));
        }
        self.learned_branch_id = Some(request.branch_id);
        let mut body = vec![1, 0];
        crate::write_string16(&self.xid, &mut body)?;
        body.extend_from_slice(&request.branch_id.to_be_bytes());
        body.push(8);
        let mut frame = crate::build_v1_request(6, request.request_id, &body)?;
        frame[9] = crate::RESPONSE;
        if !self.completed {
            apply()?;
            self.completed = true;
        }
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::PendingBranch;
    use crate::branch_end_request::BranchEndRequest;
    fn request() -> BranchEndRequest {
        BranchEndRequest {
            request_id: 77,
            kind: 5,
            xid: "x".into(),
            branch_id: 42,
            branch_type: 0,
            resource: "r".into(),
            application_data: "nonce".into(),
        }
    }
    #[test]
    fn learns_only_from_validated_callback_and_caches_simulated_success() {
        let mut state = PendingBranch::new("x", "r", "nonce");
        let mut actions = 0;
        assert_eq!(state.learned_branch_id, None);
        for id in [77, 78] {
            let mut callback = request();
            callback.request_id = id;
            let frame = state
                .reply(&callback, || {
                    actions += 1;
                    Ok(())
                })
                .unwrap();
            assert_eq!(&frame[12..16], &id.to_be_bytes());
            assert_eq!(
                &frame[16..],
                &[0, 6, 1, 0, 0, 1, b'x', 0, 0, 0, 0, 0, 0, 0, 42, 8]
            );
        }
        assert_eq!(state.learned_branch_id, Some(42));
        assert_eq!(actions, 1);
    }
    #[test]
    fn mismatched_pending_identity_never_learns_or_applies() {
        for field in 0..6 {
            let mut state = PendingBranch::new("x", "r", "nonce");
            let mut callback = request();
            match field {
                0 => callback.kind = 3,
                1 => callback.xid = "other".into(),
                2 => callback.resource = "other".into(),
                3 => callback.branch_type = 1,
                4 => callback.branch_id = 0,
                _ => callback.application_data = "wrong nonce".into(),
            }
            let mut applied = false;
            assert!(state
                .reply(&callback, || {
                    applied = true;
                    Ok(())
                })
                .is_err());
            assert!(!applied);
            assert_eq!(state.learned_branch_id, None);
        }
    }
    #[test]
    fn conflicting_learned_id_is_rejected_and_failed_action_is_not_cached() {
        let mut state = PendingBranch::new("x", "r", "nonce");
        assert!(state
            .reply(&request(), || Err(crate::invalid("simulated failure")))
            .is_err());
        let mut conflicting = request();
        conflicting.branch_id = 43;
        assert!(state.reply(&conflicting, || Ok(())).is_err());
        let mut actions = 0;
        state
            .reply(&request(), || {
                actions += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(actions, 1);
    }

    #[test]
    fn missing_pending_nonce_cannot_admit_a_callback() {
        let mut state = PendingBranch::new("x", "r", "");
        let mut callback = request();
        callback.application_data.clear();
        let mut actions = 0;
        assert!(state
            .reply(&callback, || {
                actions += 1;
                Ok(())
            })
            .is_err());
        assert_eq!(actions, 0);
        assert_eq!(state.learned_branch_id, None);
    }
}
