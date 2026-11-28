//! 固定原JavaTC上的正式TM功能驱动；空全局事务，不声称RM或SQL/Undo验收。
use seata_protocol::v1::FrameLimits;
use seata_protocol::{GlobalStatus, PayloadLimits};
use seata_tm::{
    GlobalTransactionRole, Propagation, RootContext, TransactionCompletion, TransactionConfig,
    TransactionError, TransactionManager, Xid,
};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
};
use std::error::Error;
use std::io;
use std::time::Duration;

fn ensure(value: bool, reason: &'static str) -> Result<(), Box<dyn Error>> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(reason).into())
    }
}
fn config(name: &str, propagation: Propagation) -> Result<TransactionConfig, Box<dyn Error>> {
    Ok(TransactionConfig::builder(name.to_owned())
        .propagation(propagation)
        .build()?)
}
fn observed(
    completion: &TransactionCompletion,
    expected: GlobalStatus,
) -> Result<Xid, Box<dyn Error>> {
    match completion {
        TransactionCompletion::Observed { xid, outcome } if outcome.status() == expected => {
            Ok(xid.clone())
        }
        _ => Err(io::Error::other("unexpected real TC completion").into()),
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let endpoint = std::env::var("SEATA_TM_ENDPOINT")?.parse()?;
    let nonce = std::env::var("SEATA_TM_NONCE")?;
    let connection = ConnectionConfig::new(
        FrameLimits::new(65536, 32767, 128, PayloadLimits::new(65536, 512, 1024, 16)?)?,
        16,
        Duration::from_secs(3),
        Duration::from_secs(3),
    )?;
    let pool = ConnectionPoolConfig::new(
        endpoint,
        connection,
        1,
        16,
        ReconnectPolicy::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        )?,
    )?;
    let identity = RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some("phase0-local-only".into()),
    )?;
    let tm = TransactionManager::builder(pool, identity)
        .request_timeout(Duration::from_secs(3))
        .build()?;
    let name = |suffix: &str| format!("tm-{nonce}-{suffix}");
    let mut completed = Vec::new();
    let result = tm
        .execute(config(&name("commit"), Propagation::Required)?, || async {
            let context =
                RootContext::current().ok_or_else(|| io::Error::other("missing launcher"))?;
            ensure(
                context.role() == GlobalTransactionRole::Launcher,
                "wrong launcher role",
            )?;
            Ok::<_, Box<dyn Error>>(context.xid().clone())
        })
        .await?;
    let xid = observed(result.completion(), GlobalStatus::Committed)?;
    ensure(result.value() == &xid, "business/commit XID mismatch")?;
    ensure(
        RootContext::current().is_none(),
        "context leaked after commit",
    )?;
    println!("PASS TM_BEGIN_COMMIT xid={xid} status=9 context_clean=true");
    completed.push(xid);

    let failure = tm
        .execute(
            config(&name("rollback"), Propagation::Required)?,
            || async { Err::<(), _>("business-failed") },
        )
        .await;
    match failure {
        Err(TransactionError::Business {
            error: "business-failed",
            xid: Some(xid),
            rollback: Some(Ok(completion)),
        }) => {
            ensure(
                observed(&completion, GlobalStatus::Rollbacked)? == xid,
                "rollback identity changed",
            )?;
            println!("PASS TM_BUSINESS_ROLLBACK xid={xid} status=11 error_preserved=true");
            completed.push(xid);
        }
        _ => return Err(io::Error::other("business rollback contract failed").into()),
    }

    let joined = tm
        .execute(
            config(&name("required"), Propagation::Required)?,
            || async {
                let outer = RootContext::current()
                    .ok_or_else(|| io::Error::other("missing outer context"))?;
                for policy in [
                    Propagation::Required,
                    Propagation::Supports,
                    Propagation::Mandatory,
                ] {
                    let joined = tm
                        .execute(config(&name("join"), policy)?, || async {
                            let context = RootContext::current()
                                .ok_or_else(|| io::Error::other("missing participant"))?;
                            ensure(
                                context.xid() == outer.xid()
                                    && context.role() == GlobalTransactionRole::Participant
                                    && context.deadline() == outer.deadline(),
                                "nested participant changed identity/role/deadline",
                            )?;
                            Ok::<_, Box<dyn Error>>(())
                        })
                        .await?;
                    ensure(
                        matches!(joined.completion(), TransactionCompletion::Participant(_)),
                        "participant forged TC completion",
                    )?;
                    ensure(
                        RootContext::current().as_ref() == Some(&outer),
                        "nested scope changed outer launcher",
                    )?;
                }
                let forbidden = tm
                    .execute(config(&name("never"), Propagation::Never)?, || async {
                        Err::<(), _>("must-not-run")
                    })
                    .await;
                ensure(
                    matches!(forbidden, Err(TransactionError::Tm(_))),
                    "Never failed to reject current transaction",
                )?;
                Ok::<_, Box<dyn Error>>(())
            },
        )
        .await?;
    let xid = observed(joined.completion(), GlobalStatus::Committed)?;
    println!("PASS TM_REQUIRED_PARTICIPANTS xid={xid} joined=3 outer_launcher_preserved=true");
    completed.push(xid);

    let nested = tm
        .execute(config(&name("outer"), Propagation::Required)?, || async {
            let outer =
                RootContext::current().ok_or_else(|| io::Error::other("missing outer context"))?;
            let inner = tm
                .execute(
                    config(&name("inner"), Propagation::RequiresNew)?,
                    || async {
                        Ok::<_, io::Error>(
                            RootContext::current()
                                .ok_or_else(|| io::Error::other("missing inner context"))?
                                .xid()
                                .clone(),
                        )
                    },
                )
                .await?;
            let inner_xid = observed(inner.completion(), GlobalStatus::Committed)?;
            ensure(
                &inner_xid != outer.xid() && inner.value() == &inner_xid,
                "RequiresNew reused outer transaction",
            )?;
            let unmanaged = tm
                .execute(
                    config(&name("suspended"), Propagation::NotSupported)?,
                    || async {
                        ensure(
                            RootContext::current().is_none(),
                            "NotSupported did not suspend",
                        )?;
                        Ok::<_, Box<dyn Error>>(())
                    },
                )
                .await?;
            ensure(
                unmanaged.completion() == &TransactionCompletion::Unmanaged,
                "NotSupported created a transaction",
            )?;
            ensure(
                RootContext::current().as_ref() == Some(&outer),
                "RequiresNew did not restore outer",
            )?;
            Ok::<_, Box<dyn Error>>(inner_xid)
        })
        .await?;
    let outer = observed(nested.completion(), GlobalStatus::Committed)?;
    println!(
        "PASS TM_REQUIRES_NEW outer={outer} inner={} suspended=true",
        nested.value()
    );
    completed.push(outer);
    completed.push(nested.value().clone());

    let mut launcher = tm
        .begin(&config(&name("participant"), Propagation::Required)?)
        .await?;
    launcher
        .scope(|| async {
            let participant = tm
                .execute(
                    config(&name("participate"), Propagation::Required)?,
                    || async { Ok::<_, io::Error>(()) },
                )
                .await?;
            ensure(
                matches!(
                    participant.completion(),
                    TransactionCompletion::Participant(_)
                ),
                "participant sent second phase",
            )?;
            Ok::<_, Box<dyn Error>>(())
        })
        .await?;
    ensure(
        tm.get_status(launcher.xid()).await?.status() == GlobalStatus::Begin,
        "participant ended launcher transaction",
    )?;
    let completion = launcher.rollback().await?;
    let xid = observed(&completion, GlobalStatus::Rollbacked)?;
    println!("PASS TM_PARTICIPANT xid={xid} status_before_owner_end=1 owner_rollback=11");
    completed.push(xid);

    let deadline = TransactionConfig::builder(name("deadline"))
        .transaction_timeout(Duration::from_millis(50))
        .build()?;
    match tm
        .execute(deadline, || async {
            std::future::pending::<Result<(), &str>>().await
        })
        .await
    {
        Err(TransactionError::Deadline {
            xid,
            rollback: Ok(TransactionCompletion::Observed { outcome, .. }),
        }) => {
            ensure(
                matches!(
                    outcome.status(),
                    GlobalStatus::Rollbacked
                        | GlobalStatus::TimeoutRollbacked
                        | GlobalStatus::Finished
                ),
                "unexpected deadline cleanup observation",
            )?;
            println!("PASS TM_BUSINESS_DEADLINE xid={xid} response_status={} deadline_error_retained=true",outcome.status().to_java_v1());
            completed.push(xid);
        }
        _ => return Err(io::Error::other("business deadline not retained").into()),
    }
    ensure(RootContext::current().is_none(), "final context leaked")?;
    for xid in completed {
        let until = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let status = tm.get_status(&xid).await?.status();
            if status == GlobalStatus::Finished {
                break;
            }
            ensure(
                matches!(
                    status,
                    GlobalStatus::Committed
                        | GlobalStatus::AsyncCommitting
                        | GlobalStatus::Rollbacked
                        | GlobalStatus::TimeoutRollbacked
                ),
                "unexpected TC state during explicit test cleanup observation",
            )?;
            ensure(
                tokio::time::Instant::now() < until,
                "TC cleanup observation deadline exceeded",
            )?;
            println!("WAIT xid={xid} status={}", status.to_java_v1());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        println!("CLEAN xid={xid} status=15");
    }
    tm.shutdown().await?;
    Ok(())
}
