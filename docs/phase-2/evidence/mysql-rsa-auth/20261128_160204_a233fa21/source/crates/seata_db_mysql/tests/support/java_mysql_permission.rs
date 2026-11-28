//! 以 MySQL 真正的列授权限制原 Java TC 提交回调的终态写入。

use crate::{fixture, java_pool_probe, owned_mysql};
use seata_db_mysql::MySqlOptions;
use sqlx::MySqlConnection;
use std::path::Path;

const USER: &str = "seata_rm_terminal";

/// 在自有库创建受限账号，允许业务、见证和授权写入，仅缺少回调最后更新的列权限。
pub(super) async fn restricted_options(
    admin: &mut MySqlConnection,
    port: Option<u16>,
) -> MySqlOptions {
    assert!(port.is_some(), "the live MySQL observer is required");
    let db = std::env::var("SEATA_MYSQL_DATABASE").unwrap();
    assert!(db.starts_with("undo_") && db.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    let password = std::env::var("SEATA_MYSQL_PASSWORD").unwrap();
    assert_eq!(password, "seata-mysql-fixture-only");
    for sql in [
        format!("CREATE USER '{USER}'@'%' IDENTIFIED BY '{password}'"),
        format!("GRANT PROCESS ON *.* TO '{USER}'@'%'"),
        format!("GRANT SELECT,INSERT,DELETE,TRIGGER,CREATE TEMPORARY TABLES ON `{db}`.* TO '{USER}'@'%'"),
        format!("GRANT UPDATE ON `{db}`.seata_rs_at_commit_attempt TO '{USER}'@'%'"),
        format!("GRANT UPDATE (phase,commit_authorized,modified_at,first_phase_committed) ON `{db}`.seata_rs_at_branch_v1 TO '{USER}'@'%'"),
    ] {
        sqlx::raw_sql(&sql).execute(&mut *admin).await.unwrap();
    }
    owned_mysql::options_for_user(port, USER.into(), password)
}

/// 发布原分支身份并等待独立数据库保存回调前的 Undo 镜像摘要。
pub(super) async fn wait_for_baseline(observations: &Path, xid: &str, branch_id: i64) {
    fixture::save(
        "java-terminal-permission-ready.json",
        &serde_json::json!({"xid":xid, "branch_id":branch_id}),
    );
    java_pool_probe::wait_for_snapshot(observations, "java-terminal-permission-end-permitted")
        .await;
}
