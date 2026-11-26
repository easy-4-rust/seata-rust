//! 同一资源与持久日志的 RM 所有者关闭和重建。
use crate::{fixture, pool, registration};
use seata_db_mysql::MySqlDataSourceManager;
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResource, FileBranchStore,
    ResourceCatalog, ResourceManager,
};
use seata_tm::Xid;
use std::{sync::Arc, time::Duration};

/// 消费原管理器，等待关闭，再按原身份建立新的已登记会话。
pub(super) async fn restart(
    rm: ResourceManager,
    adapter: &Arc<MySqlDataSourceManager>,
    store: &FileBranchStore,
    admin: &mut sqlx::MySqlConnection,
    xid: &Xid,
    branch_id: i64,
) -> ResourceManager {
    let key = adapter.key().clone();
    let sampler = rm.connection_sampler();
    rm.shutdown().await.unwrap();
    assert!(sampler.snapshot().closed);
    if std::env::var("SEATA_CALLBACK_IN_FLIGHT").as_deref() == Ok("1") {
        fixture::save(
            "java-rm-owner-closed.json",
            &serde_json::json!({"xid":xid.as_str(),"branch_id":branch_id}),
        );
        let folder = std::path::PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
        crate::java_pool_probe::wait_for_snapshot(&folder, "java-rm-closed-snapshot-recorded")
            .await;
    }

    let identity = BranchIdentity::new(xid.as_str().into(), branch_id, key.clone(), None).unwrap();
    let pending = store
        .lease(
            identity,
            if std::env::var("SEATA_CALLBACK_ROLLBACK_SHUTDOWN").as_deref() == Ok("1") {
                BranchPhase::Rollback
            } else {
                BranchPhase::Commit
            },
        )
        .await
        .unwrap()
        .state()
        == Some(BranchRecordState::Pending);
    let retained: (u8, Vec<u8>, u8) = sqlx::query_as(
            "SELECT b.first_phase_committed,b.undo_sha256,a.report_started FROM seata_rs_at_branch_v1 b JOIN seata_rs_at_commit_attempt a USING(xid,branch_id) WHERE b.xid=? AND b.branch_id=?")
            .bind(xid.as_str().as_bytes()).bind(branch_id).fetch_one(&mut *admin).await.unwrap();
    assert_eq!((retained.0, retained.2), (1, 1));
    assert_eq!(retained.1.len(), 32);
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(3))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let reply = rm
        .client()
        .lock_query(seata_protocol::transaction::GlobalLockQueryRequest {
            xid: Some(xid.as_str().into()),
            branch_type: seata_protocol::BranchType::At,
            resource_id: Some(key.resource_id().into()),
            lock_key: Some("undo_work:1".into()),
            application_data: None,
        })
        .await
        .unwrap();
    assert!(reply.lockable);
    fixture::save(
        "java-graceful-rm-restart.json",
        &serde_json::json!({
            "xid":xid.as_str(),"branch_id":branch_id,"old_pool_closed":true,
            "first_phase_committed":retained.0,"report_started":retained.2,
            "undo_checksum_bytes":retained.1.len(),"replacement_lock_query":true,"pending_after_shutdown":pending
        }),
    );
    rm
}
