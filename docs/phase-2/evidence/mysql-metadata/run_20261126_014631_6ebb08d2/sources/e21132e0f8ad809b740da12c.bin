//! 正式TM作用域的真实JavaTC响应丢失驱动；RM动作只在内存中模拟。
#[path = "../../seata_transport/examples/support/simulated_branch.rs"]
mod simulated_branch;
use seata_protocol::transaction::{BranchRegisterRequest, GlobalLockQueryRequest};
use seata_protocol::v1::FrameLimits;
use seata_protocol::{BranchType, GlobalStatus, Message, PayloadLimits, ResultCode};
use seata_tm::{
    RootContext, TmError, TmOperation, TransactionCompletion, TransactionConfig, TransactionError,
    TransactionManager,
};
use seata_transport::{
    ConnectionConfig, ConnectionPool, ConnectionPoolConfig, PoolHandle, ReconnectPolicy,
    RegistrationIdentity, ResourceKey, ResourceRegistry, SessionRegistration,
};
use simulated_branch::SimulatedBranch;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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
fn connection(endpoint: SocketAddr) -> Result<ConnectionPoolConfig, Box<dyn Error>> {
    Ok(ConnectionPoolConfig::new(
        endpoint,
        ConnectionConfig::new(
            FrameLimits::new(65536, 32767, 128, PayloadLimits::new(65536, 512, 1024, 16)?)?,
            16,
            Duration::from_secs(3),
            Duration::from_secs(3),
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
fn config(name: String) -> Result<TransactionConfig, Box<dyn Error>> {
    Ok(TransactionConfig::builder(name).build()?)
}
fn unknown_id(error: &TmError, expected: TmOperation) -> Result<i32, Box<dyn Error>> {
    match error {
        TmError::UnknownOutcome {
            operation, source, ..
        } if *operation == expected => source
            .request_id()
            .ok_or_else(|| io::Error::other("original request ID missing").into()),
        _ => Err(io::Error::other("expected original typed TM unknown outcome").into()),
    }
}
fn status(completion: TransactionCompletion) -> Result<GlobalStatus, Box<dyn Error>> {
    match completion {
        TransactionCompletion::Observed { outcome, .. } => Ok(outcome.status()),
        _ => Err(io::Error::other("launcher did not return actual TC outcome").into()),
    }
}
async fn lock(handle: &PoolHandle, contender: &str, expected: bool) -> Result<(), Box<dyn Error>> {
    let reply = handle
        .request(
            Message::GlobalLockQueryRequest(GlobalLockQueryRequest {
                xid: Some(contender.into()),
                branch_type: BranchType::At,
                resource_id: Some("mysql://auth-a".into()),
                lock_key: Some("orders:1".into()),
                application_data: None,
            }),
            Duration::from_secs(2),
        )
        .await?;
    match reply.body {
        Some(Message::GlobalLockQueryResponse(value))
            if value.response.result_code == ResultCode::Success && value.lockable == expected =>
        {
            println!("LOCK contender={contender} lockable={expected} expected={expected}");
            Ok(())
        }
        _ => Err(io::Error::other("original branch lock occupation/release differs").into()),
    }
}
async fn callback(
    pool: &mut ConnectionPool,
    state: &mut SimulatedBranch,
) -> Result<i32, Box<dyn Error>> {
    let inbound = tokio::time::timeout(Duration::from_secs(8), pool.next_inbound())
        .await?
        .ok_or_else(|| io::Error::other("RM inbound ended"))?;
    let id = inbound.message().id;
    let reply = state.prepare(
        inbound
            .message()
            .body
            .as_ref()
            .ok_or_else(|| io::Error::other("callback has no body"))?,
    )?;
    inbound.respond(reply, Duration::from_secs(2)).await?;
    Ok(id)
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let endpoint: SocketAddr = std::env::var("SEATA_TM_LOSS_ENDPOINT")?.parse()?;
    let mode = std::env::var("SEATA_TM_LOSS_MODE")?;
    let nonce = std::env::var("SEATA_TM_LOSS_NONCE")?;
    ensure(
        matches!(mode.as_str(), "begin" | "commit" | "rollback"),
        "unsupported loss mode",
    )?;
    let tm = TransactionManager::builder(connection(endpoint)?, identity()?)
        .request_timeout(Duration::from_secs(2))
        .build()?;
    let resources = ResourceRegistry::new(4, 512)?;
    resources.insert(ResourceKey::new(BranchType::At, "mysql://auth-a".into())?)?;
    let mut rm = ConnectionPool::start_registered(
        connection(endpoint)?,
        SessionRegistration::rm(identity()?, resources, Duration::from_secs(4))?,
    )?;
    let rm_handle = rm.handle();
    let _ready = rm_handle.acquire(Duration::from_secs(5)).await?;
    if mode == "begin" {
        let started = AtomicBool::new(false);
        let error = match tm
            .execute(config(format!("tm-loss-{nonce}-begin"))?, || {
                started.store(true, Ordering::SeqCst);
                async { Ok::<_, Box<dyn Error>>(()) }
            })
            .await
        {
            Err(TransactionError::Tm(error)) => error,
            _ => {
                return Err(io::Error::other("lost Begin did not return original TM error").into())
            }
        };
        let id = unknown_id(&error, TmOperation::Begin)?;
        ensure(
            error.xid().is_none()
                && !started.load(Ordering::SeqCst)
                && RootContext::current().is_none(),
            "lost Begin bound XID or started business",
        )?;
        ensure(
            tm.recover_unknown(&error).is_err(),
            "Begin error minted end recovery",
        )?;
        println!("PASS TM_LOSS_BEGIN operation=1 request_id={id} xid_known=false begin_attempts=1 business_started=false unknown=true");
    } else {
        let commit = mode == "commit";
        let mut contender = tm
            .begin(&config(format!("tm-loss-{nonce}-contender"))?)
            .await?;
        let contender_xid = contender.xid().clone();
        let (ready, waiting) = tokio::sync::oneshot::channel();
        let operation = if commit {
            TmOperation::Commit
        } else {
            TmOperation::Rollback
        };
        let business = tm.execute(config(format!("tm-loss-{nonce}-target"))?, || async {
            let context = RootContext::current()
                .ok_or_else(|| io::Error::other("scoped business has no XID"))?;
            let xid = context.xid().clone();
            let application_data = format!("{{\"formalOperation\":\"{nonce}\"}}");
            let reply = rm_handle
                .request(
                    Message::BranchRegisterRequest(BranchRegisterRequest {
                        xid: Some(xid.as_str().into()),
                        branch_type: BranchType::At,
                        resource_id: Some("mysql://auth-a".into()),
                        lock_key: Some("orders:1".into()),
                        application_data: Some(application_data.clone()),
                    }),
                    Duration::from_secs(2),
                )
                .await?;
            let branch_id = match reply.body {
                Some(Message::BranchRegisterResponse(value))
                    if value.response.result_code == ResultCode::Success && value.branch_id > 0 =>
                {
                    value.branch_id
                }
                _ => {
                    return Err::<(), Box<dyn Error>>(
                        io::Error::other("branch registration failed").into(),
                    )
                }
            };
            ensure(
                tm.get_status(&xid).await?.status() == GlobalStatus::Begin,
                "target was not Begin before scoped end",
            )?;
            lock(&rm_handle, contender_xid.as_str(), false).await?;
            ready
                .send((
                    xid.clone(),
                    branch_id,
                    SimulatedBranch::new(
                        xid.as_str().into(),
                        "mysql://auth-a".into(),
                        application_data,
                        commit,
                        Some(branch_id),
                    ),
                ))
                .map_err(|_| io::Error::other("callback receiver ended"))?;
            if commit {
                Ok(())
            } else {
                Err(io::Error::other("intentional-business-failure").into())
            }
        });
        let reply = async {
            let (xid, branch_id, mut state) = waiting.await?;
            let callback_id = callback(&mut rm, &mut state).await?;
            ensure(
                state.actions() == 1 && state.branch_id() == Some(branch_id),
                "callback repeated action or changed branch ID",
            )?;
            Ok::<_, Box<dyn Error>>((xid, branch_id, callback_id, state.actions()))
        };
        let (result, reply) = tokio::join!(business, reply);
        let (xid, branch_id, callback_id, actions) = reply?;
        let error = match result {
            Err(TransactionError::Tm(error)) if commit => error,
            Err(TransactionError::Business {
                error,
                rollback: Some(Err(rollback)),
                ..
            }) if !commit && error.to_string() == "intentional-business-failure" => rollback,
            _ => {
                return Err(
                    io::Error::other("scoped end lost original error or business failure").into(),
                )
            }
        };
        let request_id = unknown_id(&error, operation)?;
        ensure(
            error.xid() == Some(&xid) && RootContext::current().is_none(),
            "unknown end lost original identity or leaked context",
        )?;
        let mut recovered = tm.recover_unknown(&error)?;
        if commit {
            ensure(
                recovered.rollback().await.is_err(),
                "unknown Commit changed to Rollback",
            )?;
            ensure(recovered.commit().await.is_err(), "retry skipped status")?;
        } else {
            ensure(
                recovered.commit().await.is_err(),
                "unknown Rollback changed to Commit",
            )?;
            ensure(recovered.rollback().await.is_err(), "retry skipped status")?;
        }
        let until = tokio::time::Instant::now() + Duration::from_secs(5);
        let observed = loop {
            let current = recovered.reconcile().await?.status();
            if matches!(
                current,
                GlobalStatus::Finished
                    | GlobalStatus::Committed
                    | GlobalStatus::Rollbacked
                    | GlobalStatus::TimeoutRollbacked
            ) {
                break current;
            }
            ensure(
                tokio::time::Instant::now() < until,
                "end status did not converge",
            )?;
            ensure(
                if commit {
                    matches!(
                        current,
                        GlobalStatus::Committing
                            | GlobalStatus::CommitRetrying
                            | GlobalStatus::AsyncCommitting
                    )
                } else {
                    matches!(
                        current,
                        GlobalStatus::Rollbacking
                            | GlobalStatus::RollbackRetrying
                            | GlobalStatus::TimeoutRollbacking
                            | GlobalStatus::TimeoutRollbackRetrying
                    )
                },
                "unexpected opposite end or operator status",
            )?;
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        let retry = if recovered.has_unknown_outcome() {
            Some(status(if commit {
                recovered.commit().await?
            } else {
                recovered.rollback().await?
            })?)
        } else {
            None
        };
        let unknown_retained = recovered.has_unknown_outcome();
        if observed == GlobalStatus::Finished && retry == Some(GlobalStatus::Finished) {
            ensure(unknown_retained, "Finished erased original uncertainty")?;
        }
        let expected = if commit {
            GlobalStatus::Committed
        } else {
            GlobalStatus::Rollbacked
        };
        ensure(
            matches!(observed, GlobalStatus::Finished)
                || observed == expected
                || (!commit && observed == GlobalStatus::TimeoutRollbacked),
            "status confirmed opposite operation",
        )?;
        if let Some(value) = retry {
            ensure(
                value == GlobalStatus::Finished
                    || value == expected
                    || (!commit && value == GlobalStatus::TimeoutRollbacked),
                "duplicate returned unexpected status",
            )?;
        }
        let attempts = if retry.is_some() { 2 } else { 1 };
        let retry_status = retry.map_or(-1, |value| i32::from(value.to_java_v1()));
        println!("PASS TM_LOSS_{} operation={} request_id={request_id} xid={xid} branch_id={branch_id} callback_id={callback_id} attempts={attempts} explicit_retry={} observed={} retry_status={retry_status} unknown_retained={unknown_retained} actions={actions} business_error_preserved={}",if commit {"COMMIT"} else {"ROLLBACK"},if commit {7}else{9},retry.is_some(),observed.to_java_v1(),!commit);
        lock(&rm_handle, contender_xid.as_str(), true).await?;
        ensure(
            status(contender.rollback().await?)? == GlobalStatus::Rollbacked,
            "contender cleanup not acknowledged",
        )?;
    }
    rm.shutdown().await?;
    tm.shutdown().await?;
    Ok(())
}
