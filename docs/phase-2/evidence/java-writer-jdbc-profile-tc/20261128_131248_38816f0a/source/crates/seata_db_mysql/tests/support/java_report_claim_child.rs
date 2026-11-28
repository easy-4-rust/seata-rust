//! 固定 Java TC 现场用例的独立恢复进程。运行器在原 RM 登记应答被截留时终止进程。
use super::{fixture, owned_mysql, pool, registration};
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_rm::{FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager};
use std::{path::Path, path::PathBuf, sync::Arc, time::Duration};

/// 将原分支身份交给运行器，阻塞到独立恢复进程已退出，再由原父进程尝试恢复。
pub(super) async fn wait_for_exit(folder: &Path, xid: &str, branch_id: i64) {
    fixture::save(
        "java-report-claim-child-ready.json",
        &serde_json::json!({"xid": xid, "branch_id": branch_id, "parent_pid": std::process::id()}),
    );
    let deployed = folder.join("java-report-claim-child-exited");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(25);
    while !deployed.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "运行器未确认恢复子进程退出"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
#[ignore = "child process supervised by the pinned Java TC test runner"]
async fn recover_report_claim_then_wait_for_rm_registration() {
    let endpoint = std::env::var("SEATA_JAVA_TC_CLAIM_CHILD_RM").unwrap();
    let port = std::env::var("SEATA_JAVA_RM_MYSQL_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let source = MySqlDataSource::connect(
        owned_mysql::options(Some(port)).with_java_tc_mysql_db_lock_profile(),
    )
    .await
    .unwrap();
    let adapter =
        Arc::new(MySqlDataSourceManager::new(source, MySqlJdbcProfile::Connector8Default).unwrap());
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let journal = FileBranchStore::open(
        folder.join("java-report-claim-child.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(pool(&endpoint), registration(), catalog, journal)
        .request_timeout(Duration::from_secs(20))
        .registration_timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    let xid = std::env::var("SEATA_JAVA_TC_CLAIM_CHILD_XID").unwrap();
    let branch_id = std::env::var("SEATA_JAVA_TC_CLAIM_CHILD_BRANCH")
        .unwrap()
        .parse()
        .unwrap();

    // 进程必须停在原报告的 RM 注册等待中，正常返回也视为夹具失败。
    let result = adapter
        .report_recovered_applied(&rm.client(), &xid, branch_id)
        .await;
    panic!("独立恢复进程在终止前返回: {result:?}");
}
