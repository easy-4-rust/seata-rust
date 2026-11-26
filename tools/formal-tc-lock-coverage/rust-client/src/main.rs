//! 原TC排他锁覆盖观察驱动；正式TM/RM与回滚式SQL镜像，不提供业务提交。
#[path = "../../../formal-rm-java/rust-client/src/fixture_resource.rs"]
mod fixture_resource;
mod native_keys;

use fixture_resource::FixtureResource;
use seata_protocol::transaction::{BranchRegisterRequest, GlobalLockQueryRequest};
use seata_protocol::v1::FrameLimits;
use seata_protocol::{BranchType, GlobalStatus, PayloadLimits};
use seata_rm::{
    FileBranchStore, JournalOptions, ResourceCatalog, ResourceClient, ResourceManager, RmCallError,
};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager, Xid};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity, ResourceKey,
};
use serde_json::{json, Value};
use std::{
    error::Error,
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

fn need(value: bool, reason: &str) -> Result<(), Box<dyn Error>> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(reason.to_owned()).into())
    }
}
fn pool(endpoint: SocketAddr) -> Result<ConnectionPoolConfig, Box<dyn Error>> {
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
fn identity() -> Result<RegistrationIdentity, Box<dyn Error>> {
    Ok(RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some("phase0-local-only".into()),
    )?)
}
async fn checkpoint(
    folder: &Path,
    index: usize,
    stage: &str,
    data: Value,
) -> Result<(), Box<dyn Error>> {
    println!(
        "CHECKPOINT {}",
        json!({"index":index,"stage":stage,"data":data})
    );
    io::stdout().flush()?;
    let ack = folder.join(format!("ack-{index}-{stage}"));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !ack.exists() {
        need(
            tokio::time::Instant::now() < deadline,
            "independent DB observer did not acknowledge checkpoint",
        )?;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn finished(tm: &TransactionManager, xid: &Xid) -> Result<(), Box<dyn Error>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let status = tm.get_status(xid).await?.status();
        if status == GlobalStatus::Finished {
            return Ok(());
        }
        need(
            tokio::time::Instant::now() < deadline,
            "original TC transaction did not finish",
        )?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
async fn register(
    client: &ResourceClient,
    xid: &Xid,
    key: &str,
    app: &str,
) -> Result<Value, Box<dyn Error>> {
    match client
        .branch_register(BranchRegisterRequest {
            xid: Some(xid.as_str().into()),
            branch_type: BranchType::At,
            resource_id: Some("mysql://auth-a".into()),
            lock_key: Some(key.into()),
            application_data: Some(app.into()),
        })
        .await
    {
        Ok(reply) => Ok(json!({"registered":true,"branch_id":reply.branch_id})),
        Err(RmCallError::DefiniteRejected { .. }) => {
            Ok(json!({"registered":false,"definite_rejected":true}))
        }
        Err(error) => Err(Box::new(error)),
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let folder = PathBuf::from(std::env::var("SEATA_TC_LOCK_DIR")?);
    let nonce = std::env::var("SEATA_TC_LOCK_NONCE")?;
    need(
        nonce.len() == 32 && nonce.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid public fixture nonce",
    )?;
    let app = format!("{{\"tcLockCoverage\":\"{nonce}\"}}");
    let cases = native_keys::prepare(&folder).await?;
    checkpoint(
        &folder,
        0,
        "provenance",
        json!({"cases":cases,"SQL_rolled_back":true}),
    )
    .await?;
    let store = FileBranchStore::open(
        folder.join("branches.log"),
        JournalOptions::new(64, 4_194_304, 65536, 64)?,
    )
    .await?;
    let key = ResourceKey::new(BranchType::At, "mysql://auth-a".into())?;
    let resource = Arc::new(FixtureResource::new(key, app.clone()));
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
    let client = rm.client();
    let mut registered = 0;
    for (index, case) in cases.iter().enumerate() {
        let name = case["case"]
            .as_str()
            .ok_or_else(|| io::Error::other("case name missing"))?;
        let cfg = |role: &str| {
            TransactionConfig::builder(format!("lock-{nonce}-{name}-{role}"))
                .transaction_timeout(Duration::from_secs(180))
                .build()
        };
        let mut owner = tm.begin(&cfg("owner")?).await?;
        let mut contender = tm.begin(&cfg("contender")?).await?;
        let owner_xid = owner.xid().clone();
        let contender_xid = contender.xid().clone();
        let original = register(
            &client,
            &owner_xid,
            case["owner_key"]
                .as_str()
                .ok_or_else(|| io::Error::other("owner key missing"))?,
            &app,
        )
        .await?;
        need(
            original["registered"] == true,
            "original owner registration failed",
        )?;
        registered += 1;
        let base = json!({"case":name,"owner_xid":owner_xid.as_str(),"contender_xid":contender_xid.as_str(),"owner":original});
        checkpoint(&folder, index, "owner", base.clone()).await?;
        let contender_key = case["contender_key"]
            .as_str()
            .ok_or_else(|| io::Error::other("contender key missing"))?;
        let lockable = client
            .lock_query(GlobalLockQueryRequest {
                xid: Some(contender_xid.as_str().into()),
                branch_type: BranchType::At,
                resource_id: Some("mysql://auth-a".into()),
                lock_key: Some(contender_key.into()),
                application_data: Some(app.clone()),
            })
            .await?
            .lockable;
        let result = register(&client, &contender_xid, contender_key, &app).await?;
        if result["registered"] == true {
            registered += 1;
        }
        let mut observation = base;
        observation["contender_lockable"] = json!(lockable);
        observation["contender"] = result;
        checkpoint(&folder, index, "contender", observation.clone()).await?;
        // 每次原全局结束只发一次；未知结果直接停止，不通过Finished补造历史成功。
        let owner_end = owner.rollback().await?;
        let contender_end = contender.rollback().await?;
        for (role, completion, xid) in [
            ("owner_end", owner_end, &owner_xid),
            ("contender_end", contender_end, &contender_xid),
        ] {
            match completion {
                TransactionCompletion::Observed {
                    xid: observed,
                    outcome,
                } if &observed == xid => {
                    observation[role] =
                        json!({"xid":xid.as_str(),"status":outcome.status().to_java_v1()});
                }
                _ => return Err(io::Error::other("original global end was not observed").into()),
            }
        }
        finished(&tm, &owner_xid).await?;
        finished(&tm, &contender_xid).await?;
        checkpoint(&folder, index, "released", observation).await?;
    }
    tm.shutdown().await?;
    rm.shutdown().await?;
    need(
        resource.apply_calls() == registered && resource.effect_count()? == registered as usize,
        "callback fixture effects differ from actual successful registrations",
    )?;
    println!(
        "RESULT {}",
        json!({"cases":cases.len(),"registered":registered,"callback_apply_calls":resource.apply_calls(),"fixture_effects":resource.effect_count()?,"recover_calls":resource.recover_calls(),"business_commit":false})
    );
    Ok(())
}
