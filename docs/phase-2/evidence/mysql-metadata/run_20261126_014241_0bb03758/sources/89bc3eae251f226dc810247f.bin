//! 正式transport对固定JavaTC公共认证fixture的联调程序；不创建业务事务或执行RM补偿。
use seata_protocol::v1::FrameLimits;
use seata_protocol::{BranchType, PayloadLimits};
use seata_transport::{
    ConnectionConfig, ConnectionPool, ConnectionPoolConfig, PoolHandle, ReconnectPolicy,
    RegistrationIdentity, RegistrationStatus, ResourceKey, ResourceRegistry, SessionRegistration,
    SessionRegistrationReport,
};
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

fn ensure(value: bool, message: &'static str) -> Result<(), Box<dyn Error>> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(message).into())
    }
}
fn identity(credential: &str) -> Result<RegistrationIdentity, Box<dyn Error>> {
    Ok(RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some(credential.into()),
    )?)
}
fn settings(endpoint: SocketAddr) -> Result<ConnectionPoolConfig, Box<dyn Error>> {
    let frames = FrameLimits::new(65536, 32767, 128, PayloadLimits::new(65536, 512, 1024, 16)?)?;
    let connection =
        ConnectionConfig::new(frames, 16, Duration::from_secs(2), Duration::from_secs(2))?;
    Ok(ConnectionPoolConfig::new(
        endpoint,
        connection,
        1,
        16,
        ReconnectPolicy::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        )?,
    )?)
}
async fn report(
    handle: &PoolHandle,
    predicate: fn(&SessionRegistrationReport) -> bool,
) -> Result<Arc<SessionRegistrationReport>, Box<dyn Error>> {
    Ok(tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = handle.snapshot();
            if let Some(value) = snapshot
                .slots
                .first()
                .and_then(|slot| slot.registration.clone())
            {
                if predicate(&value) {
                    return value;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let endpoint: SocketAddr = std::env::var("SEATA_REGISTRATION_ENDPOINT")?.parse()?;
    let config = settings(endpoint)?;
    let tm = ConnectionPool::start_registered(
        config,
        SessionRegistration::tm(identity("phase0-local-only")?, Duration::from_secs(2))?,
    )?;
    let handle = tm.handle();
    let channel = handle.acquire(Duration::from_secs(5)).await?;
    ensure(handle.snapshot().ready() == 1, "TM not ready")?;
    println!(
        "PASS TM_READY generation={} resources=0",
        channel.generation().value()
    );
    tm.shutdown().await?;

    let denied = ConnectionPool::start_registered(
        config,
        SessionRegistration::tm(identity("phase0-public-denied")?, Duration::from_secs(2))?,
    )?;
    let handle = denied.handle();
    let result = report(&handle, |report| {
        report.identity.status == RegistrationStatus::Rejected
    })
    .await?;
    ensure(
        !result.is_ready() && handle.snapshot().ready() == 0,
        "denied TM became ready",
    )?;
    println!("PASS TM_DENIED identified=false ready=false");
    denied.shutdown().await?;

    let resources = ResourceRegistry::new(8, 512)?;
    for name in ["mysql://auth-a", "mysql://auth-b"] {
        resources.insert(ResourceKey::new(BranchType::At, name.into())?)?;
    }
    let rm = ConnectionPool::start_registered(
        config,
        SessionRegistration::rm(
            identity("phase0-local-only")?,
            resources.clone(),
            Duration::from_secs(2),
        )?,
    )?;
    let handle = rm.handle();
    let old = handle.acquire(Duration::from_secs(5)).await?;
    let first = report(&handle, SessionRegistrationReport::is_ready).await?;
    ensure(
        first.resources.len() == 2,
        "initial RM resource count differs",
    )?;
    println!(
        "PASS RM_READY generation={} revision={} accepted=2",
        old.generation().value(),
        first.registry_revision
    );
    old.close();
    let replacement = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let channel = handle.acquire(Duration::from_secs(2)).await?;
            if channel.generation() != old.generation() {
                break Ok::<_, seata_transport::CallError>(channel);
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    let second = report(&handle, SessionRegistrationReport::is_ready).await?;
    ensure(
        old.snapshot().closed
            && second.generation == replacement.generation()
            && second.resources.len() == 2,
        "RM re-registration not bound to replacement",
    )?;
    println!(
        "PASS RM_RECONNECTED old={} new={} revision={} accepted=2",
        old.generation().value(),
        replacement.generation().value(),
        second.registry_revision
    );
    resources.insert(ResourceKey::new(
        BranchType::At,
        "mysql://public-denied".into(),
    )?)?;
    ensure(
        handle.snapshot().ready() == 0,
        "registry addition retained stale readiness",
    )?;
    let partial = report(&handle, |report| {
        report.resources.len() == 3
            && report.resources[2].attempt.status == RegistrationStatus::Rejected
    })
    .await?;
    ensure(
        partial.identity.status == RegistrationStatus::Accepted
            && partial.resources[..2]
                .iter()
                .all(|entry| entry.attempt.status == RegistrationStatus::Accepted)
            && !partial.is_ready(),
        "partial denial semantics differ",
    )?;
    ensure(
        handle.acquire(Duration::from_millis(150)).await.is_err() && handle.snapshot().ready() == 0,
        "partial RM became selectable",
    )?;
    println!("PASS RM_PARTIAL_DENIED accepted=2 rejected=1 ready=false");
    rm.shutdown().await?;

    let resources = ResourceRegistry::new(2, 512)?;
    resources.insert(ResourceKey::new(BranchType::At, "mysql://auth-a".into())?)?;
    let denied = ConnectionPool::start_registered(
        config,
        SessionRegistration::rm(
            identity("phase0-public-denied")?,
            resources,
            Duration::from_secs(2),
        )?,
    )?;
    let handle = denied.handle();
    let result = report(&handle, |report| {
        report.identity.status == RegistrationStatus::Rejected
    })
    .await?;
    ensure(
        result.resources.len() == 1
            && result.resources[0].attempt.status == RegistrationStatus::Rejected
            && handle.snapshot().ready() == 0,
        "RM identity denial differs",
    )?;
    println!("PASS RM_IDENTITY_DENIED identified=false ready=false");
    denied.shutdown().await?;
    Ok(())
}
