//! 原 Java TC 联试的持久 Undo 摘要破坏与恢复。
use crate::fixture;
use seata_db_mysql::{DbError, MySqlCommitRecoveryError, MySqlDataSourceManager};
use sqlx::MySqlConnection;

/// 改写等长镜像后要求恢复保持未知，恢复原字节后交回同一分支。
pub(super) async fn reject_modified_undo(
    adapter: &MySqlDataSourceManager,
    client: &seata_rm::ResourceClient,
    admin: &mut MySqlConnection,
    xid: &str,
    branch_id: i64,
) {
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let _capture =
        tracing::subscriber::set_default(crate::java_checksum_events::EventCapture(events.clone()));
    let original: Vec<u8> =
        sqlx::query_scalar("SELECT rollback_info FROM undo_log WHERE xid=? AND branch_id=?")
            .bind(xid)
            .bind(branch_id)
            .fetch_one(&mut *admin)
            .await
            .unwrap();
    let stored: Vec<u8> = sqlx::query_scalar(
        "SELECT undo_sha256 FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?",
    )
    .bind(xid.as_bytes())
    .bind(branch_id)
    .fetch_one(&mut *admin)
    .await
    .unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(stored, Sha256::digest(&original).as_slice());
    // 新行缺摘要必须保持未知；不能降级使用旧行的兼容路径。
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET undo_sha256=NULL WHERE xid=? AND branch_id=?")
        .bind(xid.as_bytes())
        .bind(branch_id)
        .execute(&mut *admin)
        .await
        .unwrap();
    assert!(matches!(
        adapter
            .report_recovered_applied(client, xid, branch_id)
            .await,
        Err(seata_db_mysql::MySqlRecoveredReportError::Recovery(
            MySqlCommitRecoveryError::Observation {
                source: DbError::UndoChecksumMismatch
            }
        ))
    ));
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET undo_sha256=? WHERE xid=? AND branch_id=?")
        .bind(&stored)
        .bind(xid.as_bytes())
        .bind(branch_id)
        .execute(&mut *admin)
        .await
        .unwrap();

    sqlx::query("UPDATE undo_log SET rollback_info=REPLACE(rollback_info,_binary'original_tc_report',_binary'modified_tc_report') WHERE xid=? AND branch_id=?")
        .bind(xid).bind(branch_id).execute(&mut *admin).await.unwrap();
    assert!(matches!(
        adapter.recover_commit_attempt(xid, branch_id).await,
        Err(MySqlCommitRecoveryError::Observation {
            source: DbError::UndoChecksumMismatch
        })
    ));

    assert!(matches!(
        adapter
            .report_recovered_applied(client, xid, branch_id)
            .await,
        Err(seata_db_mysql::MySqlRecoveredReportError::Recovery(
            MySqlCommitRecoveryError::Observation {
                source: DbError::UndoChecksumMismatch
            }
        ))
    ));
    let started: u8 = sqlx::query_scalar(
        "SELECT report_started FROM seata_rs_at_commit_attempt WHERE xid=? AND branch_id=?",
    )
    .bind(xid.as_bytes())
    .bind(branch_id)
    .fetch_one(&mut *admin)
    .await
    .unwrap();
    assert_eq!(started, 0);
    sqlx::query("UPDATE undo_log SET rollback_info=? WHERE xid=? AND branch_id=?")
        .bind(&original)
        .bind(xid)
        .bind(branch_id)
        .execute(&mut *admin)
        .await
        .unwrap();
    let rejected = events
        .lock()
        .unwrap()
        .iter()
        .filter(|event| {
            event
                .get("target")
                .is_some_and(|target| target == "seata_db_mysql::undo")
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        rejected.len(),
        3,
        "each rejected checksum observation must emit one event"
    );
    for event in &rejected {
        assert_eq!(event.get("xid").map(String::as_str), Some(xid));
        assert_eq!(event.get("branch_id"), Some(&branch_id.to_string()));
        assert_eq!(event.get("outcome").map(String::as_str), Some("unknown"));
        assert_eq!(event.get("resource_digest").unwrap().len(), 64);
        assert_eq!(event.len(), 6);
        let text = format!("{event:?}");
        assert!(!text.contains("original_tc_report") && !text.contains("modified_tc_report"));
        assert!(!text.contains("jdbc:mysql") && !text.contains("seata-mysql-fixture-only"));
    }
    fixture::save(
        "java-undo-checksum-events.json",
        &serde_json::json!(rejected),
    );
    fixture::save(
        "java-undo-checksum.json",
        &serde_json::json!({
            "xid":xid,"branch_id":branch_id,"modified_recovery_rejected":true,
            "missing_checksum_report_rejected":true,"modified_undo_report_rejected":true,
            "original_bytes_restored":true,"sha256_bytes":stored.len()
        }),
    );
}
