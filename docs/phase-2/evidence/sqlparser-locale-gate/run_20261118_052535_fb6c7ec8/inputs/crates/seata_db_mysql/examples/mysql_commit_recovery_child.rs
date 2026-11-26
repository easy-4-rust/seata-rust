//! 自有数据库提交清理恢复观察子进程；读取真实 Pending 文件并走正式分派器，不重新注册或重放业务。
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager, MySqlOptions};
use seata_rm::{
    BranchDispatcher, BranchIdentity, BranchPhase, BranchRecordState, BranchResource,
    FileBranchStore, JournalOptions, ResourceCatalog,
};
use std::{error::Error, path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(std::env::var("SEATA_RM_CHILD_JOURNAL")?);
    let output = PathBuf::from(std::env::var("SEATA_RM_CHILD_OUT")?);
    if [&path, &output]
        .iter()
        .any(|p| !p.is_absolute() || !p.to_string_lossy().starts_with("D:"))
    {
        return Err("owned D paths required".into());
    }
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT")?.parse()?,
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD")?,
        std::env::var("SEATA_MYSQL_DATABASE")?,
    )?
    .with_insecure_loopback_tls()?;
    let ds = MySqlDataSource::connect(options).await?;
    let adapter = Arc::new(MySqlDataSourceManager::new(
        ds.clone(),
        MySqlJdbcProfile::Connector8Default,
    )?);
    let id = BranchIdentity::new("rollback:xid".into(), 42, adapter.key().clone(), None)?;
    let store = FileBranchStore::open(path, JournalOptions::new(16, 1048576, 65536, 16)?).await?;
    let lease = store.lease(id.clone(), BranchPhase::Commit).await?;
    if lease.state() != Some(BranchRecordState::Pending) {
        return Err("actual persisted Pending required".into());
    }
    drop(lease);
    let catalog = ResourceCatalog::new(4, 32767)?;
    catalog.register(adapter)?;
    let status = BranchDispatcher::new(catalog, store.clone())
        .dispatch(id.clone(), BranchPhase::Commit)
        .await?;
    if status != seata_protocol::BranchStatus::PhaseTwoCommitted {
        return Err("persisted commit cleanup did not complete".into());
    }
    let lease = store.lease(id, BranchPhase::Commit).await?;
    if lease.state() != Some(BranchRecordState::Completed) {
        return Err("Completed must be durably published".into());
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"pid":std::process::id(),"before":"Pending","after":"Completed","branch_status":"PhaseTwoCommitted","business_replayed":false}),
        )?,
    )?;
    ds.close().await?;
    Ok(())
}
