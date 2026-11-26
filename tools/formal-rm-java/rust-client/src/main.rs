//! 正式TM/RM对原JavaTC的独立验证驱动；资源夹具不代表SQL或Undo实现。
mod fixture_resource;

use fixture_resource::FixtureResource;
use seata_protocol::transaction::{
    BranchRegisterRequest, BranchReportRequest, GlobalLockQueryRequest,
};
use seata_protocol::v1::FrameLimits;
use seata_protocol::{BranchStatus, BranchType, GlobalStatus, PayloadLimits};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, FileBranchStore, JournalOptions,
    ResourceCatalog, ResourceClient, ResourceManager,
};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager, Xid};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity, ResourceKey,
};
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn ensure(value: bool, reason: &'static str) -> Result<(), Box<dyn Error>> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(reason).into())
    }
}
fn pool(endpoint: SocketAddr) -> Result<ConnectionPoolConfig, Box<dyn Error>> {
    let connection = ConnectionConfig::new(
        FrameLimits::new(65536, 32767, 128, PayloadLimits::new(65536, 512, 1024, 16)?)?,
        16,
        Duration::from_secs(3),
        Duration::from_secs(3),
    )?;
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
fn identity() -> Result<RegistrationIdentity, Box<dyn Error>> {
    Ok(RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some("phase0-local-only".into()),
    )?)
}
async fn lock(client: &ResourceClient, xid: &Xid, app: &str) -> Result<bool, Box<dyn Error>> {
    Ok(client
        .lock_query(GlobalLockQueryRequest {
            xid: Some(xid.as_str().into()),
            branch_type: BranchType::At,
            resource_id: Some("mysql://auth-a".into()),
            lock_key: Some("orders:1".into()),
            application_data: Some(app.into()),
        })
        .await?
        .lockable)
}
async fn finished(tm: &TransactionManager, xid: &Xid) -> Result<(), Box<dyn Error>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let value = tm.get_status(xid).await?.status();
        println!("OBSERVE xid={xid} status={}", value.to_java_v1());
        if value == GlobalStatus::Finished {
            return Ok(());
        }
        ensure(
            !matches!(
                value,
                GlobalStatus::CommitRetryTimeout
                    | GlobalStatus::RollbackRetryTimeout
                    | GlobalStatus::StopCommitOrCommitRetry
                    | GlobalStatus::StopRollbackOrRollbackRetry
            ),
            "TC requires operator handling",
        )?;
        ensure(
            tokio::time::Instant::now() < deadline,
            "TC did not finish original transaction in observation budget",
        )?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mode = std::env::var("SEATA_RM_MODE")?;
    ensure(
        matches!(mode.as_str(), "commit" | "rollback"),
        "invalid test mode",
    )?;
    let nonce = std::env::var("SEATA_RM_NONCE")?;
    ensure(
        nonce.len() == 32 && nonce.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid public fixture nonce",
    )?;
    let app = format!("{{\"formalRm\":\"{nonce}\"}}");
    let folder = PathBuf::from(
        std::env::var_os("SEATA_RM_TEST_DIR")
            .ok_or_else(|| io::Error::other("missing owned test directory"))?,
    );
    let journal_path = folder.join("branches.log");
    let options = JournalOptions::new(16, 1048576, 65536, 16)?;
    let store = FileBranchStore::open(journal_path.clone(), options).await?;
    let key = ResourceKey::new(BranchType::At, "mysql://auth-a".into())?;
    let resource = Arc::new(FixtureResource::new(key.clone(), app.clone()));
    let catalog = ResourceCatalog::new(4, 4096)?;
    catalog.register(resource.clone())?;
    let rm = ResourceManager::builder(
        pool(std::env::var("SEATA_RM_ENDPOINT")?.parse()?)?,
        identity()?,
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(5))
    .callback_timeout(Duration::from_secs(3))
    .build()?;
    let tm = TransactionManager::builder(
        pool(std::env::var("SEATA_TM_ENDPOINT")?.parse()?)?,
        identity()?,
    )
    .request_timeout(Duration::from_secs(5))
    .build()?;
    let cfg = |suffix: &str| {
        TransactionConfig::builder(format!("rm-{nonce}-{suffix}"))
            .transaction_timeout(Duration::from_secs(60))
            .build()
    };
    let mut original = tm.begin(&cfg("resource")?).await?;
    let mut contender = tm.begin(&cfg("contender")?).await?;
    let xid = original.xid().clone();
    let contender_xid = contender.xid().clone();
    let client = rm.client();
    let branch = client
        .branch_register(BranchRegisterRequest {
            xid: Some(xid.as_str().into()),
            branch_type: BranchType::At,
            resource_id: Some(key.resource_id().into()),
            lock_key: Some("orders:1".into()),
            application_data: Some(app.clone()),
        })
        .await?;
    client
        .branch_report(BranchReportRequest {
            xid: Some(xid.as_str().into()),
            branch_id: branch.branch_id,
            status: BranchStatus::PhaseOneDone,
            resource_id: None,
            application_data: Some(app.clone()),
            branch_type: BranchType::At,
        })
        .await?;
    ensure(
        lock(&client, &xid, &app).await?,
        "original owner could not observe its lock",
    )?;
    ensure(
        !lock(&client, &contender_xid, &app).await?,
        "contender did not observe lock conflict",
    )?;
    let end = if mode == "commit" {
        original.commit().await
    } else {
        original.rollback().await
    };
    match &end {
        Ok(TransactionCompletion::Observed { outcome, .. }) => println!(
            "END mode={mode} xid={xid} status={} unknown=false",
            outcome.status().to_java_v1()
        ),
        Err(error) if error.is_unknown_outcome() => {
            println!("END mode={mode} xid={xid} status=unknown unknown=true")
        }
        other => {
            return Err(io::Error::other(format!("unexpected end operation: {other:?}")).into())
        }
    };
    // 只查询TC，不重复Begin/BranchRegister/Commit/Rollback；由TC拥有原分支重试。
    finished(&tm, &xid).await?;
    ensure(
        lock(&client, &contender_xid, &app).await?,
        "original lock was not released",
    )?;
    contender.rollback().await?;
    finished(&tm, &contender_xid).await?;
    let expected = if mode == "commit" {
        BranchPhase::Commit
    } else {
        BranchPhase::Rollback
    };
    let original_identity =
        BranchIdentity::new(xid.as_str().into(), branch.branch_id, key, Some(app))?;
    {
        let lease = store.lease(original_identity.clone(), expected).await?;
        ensure(
            lease.state() == Some(BranchRecordState::Completed),
            "native callback completion was not durably recorded",
        )?;
    }
    ensure(
        resource.apply_calls() == 1,
        "resource apply was repeated or never invoked",
    )?;
    ensure(
        resource.effect_count()? == 1,
        "fixture effect count differs",
    )?;
    drop(original);
    drop(contender);
    tm.shutdown().await?;
    rm.shutdown().await?;
    drop(store);
    let reopened = FileBranchStore::open(journal_path, options).await?;
    ensure(
        reopened.lease(original_identity, expected).await?.state()
            == Some(BranchRecordState::Completed),
        "completion did not survive journal close/reopen",
    )?;
    println!("PASS RM_NATIVE mode={mode} xid={xid} branch_id={} contender={contender_xid} nonce={nonce} apply_calls={} effects={} recover_calls={} journal_reopened=true lock_before=false lock_after=true",branch.branch_id,resource.apply_calls(),resource.effect_count()?,resource.recover_calls());
    Ok(())
}
