//! 正式transport故障联调；分支动作仅内存模拟，严格区分观察器oracle与客户端可知信息。
#[path = "support/simulated_branch.rs"]
mod simulated_branch;
#[path = "support/uncertain_commit.rs"]
mod uncertain_commit;
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::{
    BranchRegisterRequest, GlobalBeginRequest, GlobalCommitRequest, GlobalLockQueryRequest,
    GlobalRollbackRequest, GlobalStatusRequest,
};
use seata_protocol::v1::FrameLimits;
use seata_protocol::{BranchType, GlobalStatus, Message, PayloadLimits, ResultCode};
use seata_transport::{
    CallError, ConnectionConfig, ConnectionPool, ConnectionPoolConfig, PoolHandle, ReconnectPolicy,
    RegistrationIdentity, ResourceKey, ResourceRegistry, SessionRegistration,
};
use simulated_branch::SimulatedBranch;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use uncertain_commit::UncertainCommit;

fn ensure(value: bool, reason: &'static str) -> Result<(), Box<dyn Error>> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(reason).into())
    }
}
fn identity() -> Result<RegistrationIdentity, Box<dyn Error>> {
    Ok(RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some("phase0-local-only".into()),
    )?)
}
fn configuration(endpoint: SocketAddr) -> Result<ConnectionPoolConfig, Box<dyn Error>> {
    Ok(ConnectionPoolConfig::new(
        endpoint,
        ConnectionConfig::new(
            FrameLimits::new(65536, 32767, 128, PayloadLimits::new(65536, 512, 1024, 16)?)?,
            16,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )?,
        1,
        16,
        ReconnectPolicy::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        )?,
    )?)
}
fn begin_body(name: &str) -> Message {
    Message::GlobalBeginRequest(GlobalBeginRequest {
        timeout: 60000,
        transaction_name: Some(name.into()),
    })
}
fn unknown(result: Result<RpcMessage, CallError>) -> Result<CallError, Box<dyn Error>> {
    match result {
        Err(error) if error.is_unknown_outcome() => Ok(error),
        _ => Err(io::Error::other("loss scenario did not retain a typed unknown outcome").into()),
    }
}
fn begin_xid(frame: RpcMessage) -> Result<String, Box<dyn Error>> {
    match frame.body {
        Some(Message::GlobalBeginResponse(value))
            if value.response.result_code == ResultCode::Success =>
        {
            match value.xid {
                Some(xid) if !xid.is_empty() => Ok(xid),
                _ => Err(io::Error::other("begin returned no XID").into()),
            }
        }
        _ => Err(io::Error::other("begin did not succeed").into()),
    }
}
async fn begin(handle: &PoolHandle, name: &str) -> Result<String, Box<dyn Error>> {
    begin_xid(
        handle
            .request(begin_body(name), Duration::from_secs(2))
            .await?,
    )
}
fn branch_body(xid: &str, resource: &str, data: &str) -> Message {
    Message::BranchRegisterRequest(BranchRegisterRequest {
        xid: Some(xid.into()),
        branch_type: BranchType::At,
        resource_id: Some(resource.into()),
        lock_key: Some("orders:1".into()),
        application_data: Some(data.into()),
    })
}
async fn status(handle: &PoolHandle, xid: &str) -> Result<GlobalStatus, Box<dyn Error>> {
    match handle
        .request(
            Message::GlobalStatusRequest(GlobalStatusRequest {
                xid: Some(xid.into()),
                extra_data: None,
            }),
            Duration::from_secs(2),
        )
        .await?
        .body
    {
        Some(Message::GlobalStatusResponse(value))
            if value.response.result_code == ResultCode::Success =>
        {
            Ok(value.global_status)
        }
        _ => Err(io::Error::other("status response is not authoritative success").into()),
    }
}
async fn lock(
    handle: &PoolHandle,
    contender: &str,
    resource: &str,
    expected: bool,
) -> Result<(), Box<dyn Error>> {
    let frame = handle
        .request(
            Message::GlobalLockQueryRequest(GlobalLockQueryRequest {
                xid: Some(contender.into()),
                branch_type: BranchType::At,
                resource_id: Some(resource.into()),
                lock_key: Some("orders:1".into()),
                application_data: None,
            }),
            Duration::from_secs(2),
        )
        .await?;
    match frame.body {
        Some(Message::GlobalLockQueryResponse(value)) => {
            ensure(
                value.response.result_code == ResultCode::Success && value.lockable == expected,
                "lock occupation/release differs",
            )?;
            println!(
                "LOCK contender={contender} lockable={} expected={expected}",
                value.lockable
            );
            Ok(())
        }
        _ => Err(io::Error::other("wrong lock response").into()),
    }
}
async fn rollback(handle: &PoolHandle, xid: &str) -> Result<GlobalStatus, Box<dyn Error>> {
    match handle
        .request(
            Message::GlobalRollbackRequest(GlobalRollbackRequest {
                xid: Some(xid.into()),
                extra_data: None,
            }),
            Duration::from_secs(3),
        )
        .await?
        .body
    {
        Some(Message::GlobalRollbackResponse(value))
            if value.response.result_code == ResultCode::Success =>
        {
            Ok(value.global_status)
        }
        _ => Err(io::Error::other("rollback did not return success").into()),
    }
}
async fn callback(
    pool: &mut ConnectionPool,
    state: &mut SimulatedBranch,
) -> Result<i32, Box<dyn Error>> {
    let inbound = tokio::time::timeout(Duration::from_secs(5), pool.next_inbound())
        .await?
        .ok_or_else(|| io::Error::other("RM inbound queue ended"))?;
    let id = inbound.message().id;
    let body = inbound
        .message()
        .body
        .as_ref()
        .ok_or_else(|| io::Error::other("callback has no body"))?;
    let response = state.prepare(body)?;
    inbound.respond(response, Duration::from_secs(1)).await?;
    Ok(id)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mode = std::env::var("SEATA_LOSS_MODE")?;
    let endpoint: SocketAddr = std::env::var("SEATA_LOSS_ENDPOINT")?.parse()?;
    let nonce = std::env::var("SEATA_LOSS_NONCE")?;
    ensure(
        matches!(mode.as_str(), "begin" | "commit" | "branch-register"),
        "unsupported loss mode",
    )?;
    let settings = configuration(endpoint)?;
    let tm = ConnectionPool::start_registered(
        settings,
        SessionRegistration::tm(identity()?, Duration::from_secs(2))?,
    )?;
    let resources = ResourceRegistry::new(2, 512)?;
    let resource = "mysql://auth-a";
    resources.insert(ResourceKey::new(BranchType::At, resource.into())?)?;
    let mut rm = ConnectionPool::start_registered(
        settings,
        SessionRegistration::rm(identity()?, resources, Duration::from_secs(2))?,
    )?;
    let th = tm.handle();
    let rh = rm.handle();
    let tc = th.acquire(Duration::from_secs(5)).await?;
    let rc = rh.acquire(Duration::from_secs(5)).await?;
    if mode == "begin" {
        let error = unknown(
            th.request(
                begin_body(&format!("formal-begin-{nonce}")),
                Duration::from_millis(500),
            )
            .await,
        )?;
        let id = error
            .request_id()
            .ok_or_else(|| io::Error::other("missing original Begin request ID"))?;
        ensure(tc.snapshot().pending == 0, "Begin waiter leaked")?;
        println!("PASS LOSS_BEGIN operation=1 request_id={id} xid_known=false begin_attempts=1 pending=0 unknown=true");
    } else {
        let xid = begin(&th, &format!("formal-target-{nonce}")).await?;
        let data = format!("{{\"formalOperation\":\"{nonce}\"}}");
        let lost_branch = mode == "branch-register";
        let mut original_unknown = None;
        let known = if lost_branch {
            original_unknown = Some(unknown(
                rh.request(
                    branch_body(&xid, resource, &data),
                    Duration::from_millis(500),
                )
                .await,
            )?);
            None
        } else {
            match rh
                .request(branch_body(&xid, resource, &data), Duration::from_secs(2))
                .await?
                .body
            {
                Some(Message::BranchRegisterResponse(value))
                    if value.response.result_code == ResultCode::Success && value.branch_id > 0 =>
                {
                    Some(value.branch_id)
                }
                _ => return Err(io::Error::other("branch registration failed").into()),
            }
        };
        ensure(
            status(&th, &xid).await? == GlobalStatus::Begin,
            "target was not in Begin before explicit end",
        )?;
        let contender = begin(&th, &format!("formal-lock-observer-{nonce}")).await?;
        lock(&rh, &contender, resource, false).await?;
        let mut simulated =
            SimulatedBranch::new(xid.clone(), resource.into(), data, !lost_branch, known);
        if lost_branch {
            ensure(
                simulated.branch_id().is_none(),
                "lost BranchId leaked from observer",
            )?;
            let (cancel, reply) =
                tokio::join!(rollback(&th, &xid), callback(&mut rm, &mut simulated));
            ensure(
                cancel? == GlobalStatus::Rollbacked,
                "pending branch cancellation not acknowledged",
            )?;
            let callback_id = reply?;
            let learned = simulated
                .branch_id()
                .ok_or_else(|| io::Error::other("callback did not reconcile BranchId"))?;
            let unknown = original_unknown
                .as_ref()
                .ok_or_else(|| io::Error::other("registration uncertainty missing"))?;
            ensure(simulated.actions() == 1, "branch action repeated")?;
            println!("PASS LOSS_BRANCH_REGISTER operation=11 request_id={} xid={xid} branch_id={learned} callback_id={callback_id} learned_from_callback=true registration_attempts=1 actions=1 cancellation_acknowledged=true",unknown.request_id().ok_or_else(||io::Error::other("missing registration request ID"))?);
        } else {
            let commit = Message::GlobalCommitRequest(GlobalCommitRequest {
                xid: Some(xid.clone()),
                extra_data: None,
            });
            let (result, reply) = tokio::join!(
                th.request(commit.clone(), Duration::from_millis(500)),
                callback(&mut rm, &mut simulated)
            );
            let error = unknown(result)?;
            let callback_id = reply?;
            let mut uncertain = UncertainCommit::new();
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            let observed = loop {
                let value = status(&th, &xid).await?;
                uncertain.observe(value);
                if matches!(value, GlobalStatus::Finished | GlobalStatus::Committed) {
                    break value;
                }
                ensure(
                    matches!(
                        value,
                        GlobalStatus::Committing
                            | GlobalStatus::CommitRetrying
                            | GlobalStatus::AsyncCommitting
                    ) && tokio::time::Instant::now() < deadline,
                    "commit status did not converge",
                )?;
                tokio::time::sleep(Duration::from_millis(20)).await;
            };
            // 调用方明确选择原XID/原操作的单次重复；transport没有任何自动事务重发。
            let retry = match th.request(commit, Duration::from_secs(2)).await?.body {
                Some(Message::GlobalCommitResponse(value))
                    if value.response.result_code == ResultCode::Success =>
                {
                    value.global_status
                }
                _ => return Err(io::Error::other("explicit same-XID commit retry failed").into()),
            };
            ensure(
                matches!(retry, GlobalStatus::Committed | GlobalStatus::Finished),
                "unexpected duplicate commit result",
            )?;
            uncertain.observe(retry);
            if observed == GlobalStatus::Finished && retry == GlobalStatus::Finished {
                ensure(
                    !uncertain.confirmed(),
                    "Finished incorrectly proved historical commit",
                )?;
            }
            ensure(
                simulated.actions() == 1,
                "duplicate Commit repeated branch effect",
            )?;
            println!("PASS LOSS_COMMIT operation=7 request_id={} xid={xid} branch_id={} callback_id={callback_id} commit_attempts=2 explicit_retry=true observed={} retry_status={} confirmed={} actions=1",error.request_id().ok_or_else(||io::Error::other("missing commit request ID"))?,simulated.branch_id().ok_or_else(||io::Error::other("known branch disappeared"))?,observed.to_java_v1(),retry.to_java_v1(),uncertain.confirmed());
        }
        lock(&rh, &contender, resource, true).await?;
        ensure(
            rollback(&th, &contender).await? == GlobalStatus::Rollbacked,
            "contender cleanup was not acknowledged",
        )?;
        ensure(
            tc.snapshot().pending == 0 && rc.snapshot().pending == 0,
            "loss scenario waiter leaked",
        )?;
    }
    rm.shutdown().await?;
    tm.shutdown().await?;
    Ok(())
}
