use crate::{BranchDispatcher, BranchIdentity, BranchKey, BranchPhase, RmError};
use seata_protocol::protocol::MergeResultMessage;
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchCommitResponse, BranchRollbackResponse,
};
use seata_protocol::{BranchStatus, Message, RequestType, ResultCode, TransactionExceptionCode};
use seata_transport::PoolInbound;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;

/// 原连接上的二阶段回调处理器；先校验整批形状/身份，再调用持久资源分派。
/// 对应 Java: AbstractRMHandler与BranchCommit/BranchRollback处理职责。
#[derive(Debug, Clone)]
pub(crate) struct CallbackHandler {
    pub(crate) dispatcher: BranchDispatcher,
    pub(crate) maximum_batch: usize,
    pub(crate) callback_timeout: Duration,
    pub(crate) response_timeout: Duration,
}
impl CallbackHandler {
    /// 消费原连接能力；参数携带原signed ID和可空有序头，绝不从池选择替代回复连接。
    pub(crate) async fn handle(&self, inbound: PoolInbound) {
        let Some(deadline) = Instant::now().checked_add(self.callback_timeout) else {
            inbound.connection().close();
            return;
        };
        let request = inbound.message();
        if request.message_type != RequestType::RequestSync {
            inbound.connection().close();
            return;
        }
        let Some(body) = request.body.as_ref() else {
            inbound.connection().close();
            return;
        };
        let Ok((requests, batch)) = parse(body, self.maximum_batch, deadline) else {
            inbound.connection().close();
            return;
        };
        let mut responses = Vec::with_capacity(requests.len());
        for (identity, phase) in requests {
            let outcome = if Instant::now() >= deadline {
                Err(RmError::Unavailable {
                    reason: "callback deadline expired",
                })
            } else {
                match tokio::time::timeout_at(
                    deadline,
                    self.dispatcher.dispatch(identity.clone(), phase),
                )
                .await
                {
                    Ok(value) => value,
                    Err(_) => Err(RmError::Unavailable {
                        reason: "callback outcome not confirmed before deadline",
                    }),
                }
            };
            let (response, branch_status) = match outcome {
                Ok(status) => (
                    AbstractTransactionResponse {
                        result_code: ResultCode::Success,
                        msg: None,
                        transaction_exception_code: TransactionExceptionCode::Unknown,
                    },
                    status,
                ),
                Err(error) => {
                    let conflict = matches!(error, RmError::IdentityConflict);
                    let status = match (phase, conflict) {
                        (BranchPhase::Commit, true) => {
                            BranchStatus::PhaseTwoCommitFailedUnretryable
                        }
                        (BranchPhase::Rollback, true) => {
                            BranchStatus::PhaseTwoRollbackFailedUnretryable
                        }
                        (BranchPhase::Commit, false) => BranchStatus::PhaseTwoCommitFailedRetryable,
                        (BranchPhase::Rollback, false) => {
                            BranchStatus::PhaseTwoRollbackFailedRetryable
                        }
                    };
                    (
                        AbstractTransactionResponse {
                            result_code: ResultCode::Failed,
                            msg: Some(
                                if conflict {
                                    "branch identity or phase conflict"
                                } else {
                                    "resource action not confirmed"
                                }
                                .into(),
                            ),
                            transaction_exception_code: TransactionExceptionCode::Unknown,
                        },
                        status,
                    )
                }
            };
            responses.push(match phase {
                BranchPhase::Commit => Message::BranchCommitResponse(BranchCommitResponse {
                    response,
                    xid: Some(identity.xid().into()),
                    branch_id: identity.branch_id(),
                    branch_status,
                }),
                BranchPhase::Rollback => Message::BranchRollbackResponse(BranchRollbackResponse {
                    response,
                    xid: Some(identity.xid().into()),
                    branch_id: identity.branch_id(),
                    branch_status,
                }),
            });
        }
        let body = if batch {
            Message::MergeResultMessage(MergeResultMessage {
                msgs: Some(responses),
            })
        } else {
            let Some(value) = responses.pop() else {
                inbound.connection().close();
                return;
            };
            value
        };
        // respond消费原能力，传输层再次核对消息族和完整分支身份；本地写完成不等于TC已确认。
        let _ = inbound.respond(body, self.response_timeout).await;
    }
}
fn parse(
    body: &Message,
    maximum: usize,
    deadline: Instant,
) -> Result<(Vec<(BranchIdentity, BranchPhase)>, bool), RmError> {
    let (messages, batch) = match body {
        Message::MergedWarpMessage(value) => {
            if value.msgs.is_empty()
                || value.msgs.len() > maximum
                || (!value.msg_ids.is_empty() && value.msg_ids.len() != value.msgs.len())
            {
                return Err(RmError::Invalid {
                    reason: "invalid flat callback batch",
                });
            }
            (value.msgs.as_slice(), true)
        }
        _ => (std::slice::from_ref(body), false),
    };
    let mut requests: Vec<(BranchIdentity, BranchPhase)> = Vec::with_capacity(messages.len());
    let mut seen: HashMap<BranchKey, usize> = HashMap::new();
    for message in messages {

        let (xid, id, kind, resource, app, phase) = match message {
            Message::BranchCommitRequest(v) => (
                &v.xid,
                v.branch_id,
                v.branch_type,
                &v.resource_id,
                v.application_data.as_deref(),
                BranchPhase::Commit,
            ),
            Message::BranchRollbackRequest(v) => (
                &v.xid,
                v.branch_id,
                v.branch_type,
                &v.resource_id,
                v.application_data.as_deref(),
                BranchPhase::Rollback,
            ),
            _ => {
                return Err(RmError::Invalid {
                    reason: "unsupported or nested callback family",
                })
            }
        };
        let identity = BranchIdentity::from_fields(xid, id, kind, resource, app)?;
        if let Some(index) = seen.get(&identity.key()) {
            let (previous, direction) = &requests[*index];
            if previous != &identity || *direction != phase {
                return Err(RmError::IdentityConflict);
            }
        }
        seen.insert(identity.key(), requests.len());
        requests.push((identity, phase));
    }
    Ok((requests, batch))
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::RmError;
    use seata_protocol::transaction::BranchCommitRequest;
    use seata_protocol::{BranchType, Message};
    use std::time::Duration;
    use tokio::time::Instant;
    fn request(app: Option<String>) -> Message {
        Message::BranchCommitRequest(BranchCommitRequest {
            xid: "tc:1".into(),
            branch_id: 1,
            branch_type: BranchType::At,
            resource_id: "fixture".into(),
            application_data: app,
        })
    }
    #[tokio::test(start_paused = true)]
    async fn validation_uses_the_original_absolute_deadline_instead_of_starting_a_new_budget() {
        let deadline = Instant::now() + Duration::from_millis(1);
        tokio::time::advance(Duration::from_millis(2)).await;
        assert!(matches!(
            parse(&request(None), 1, deadline),
            Err(RmError::Unavailable { .. })
        ));
    }
    #[test]
    fn application_data_bounds_and_some_empty_are_checked_before_identity_construction() {
        for app in [Some(String::new()), Some("x".repeat(1_048_577))] {
            let value = request(app);
            assert!(matches!(
                parse(&value, 1, Instant::now() + Duration::from_secs(1)),
                Err(RmError::Invalid { .. })
            ));
        }
    }
}
