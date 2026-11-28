//! 固定原 Java TC 联试中通过真实行锁触发 004 恢复查询超时。

use crate::{fixture, owned_mysql};
use seata_db_mysql::{DbError, MySqlCommitRecoveryError, MySqlDataSourceManager};
use sqlx::Connection;
use std::{path::Path, time::Duration};

/// 锁住指定原分支的 004 行，验证恢复返回 Unknown，并等候独立快照后释放锁。
pub(super) async fn exercise(
    adapter: &MySqlDataSourceManager,
    xid: &str,
    branch_id: i64,
    observations: &Path,
) {
    let mut admin = owned_mysql::admin().await;
    let blocker_id: i64 = sqlx::query_scalar("SELECT CAST(CONNECTION_ID() AS SIGNED)")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    let mut blocker = admin.begin().await.unwrap();
    let witness: u8 = sqlx::query_scalar(
        "SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=? FOR UPDATE",
    )
    .bind(xid.as_bytes())
    .bind(branch_id)
    .fetch_one(&mut *blocker)
    .await
    .unwrap();
    assert_eq!(witness, 0);

    let mut observer = owned_mysql::admin().await;
    let wait_for_lock = async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
        loop {
            let wait: Option<(i64, i64)> = sqlx::query_as(
                "SELECT CAST(r.PROCESSLIST_ID AS SIGNED),CAST(w.REQUESTING_THREAD_ID AS SIGNED) \
                     FROM performance_schema.data_lock_waits w \
                     JOIN performance_schema.data_locks l ON l.ENGINE_LOCK_ID=w.REQUESTING_ENGINE_LOCK_ID \
                     JOIN performance_schema.threads r ON r.THREAD_ID=w.REQUESTING_THREAD_ID \
                     JOIN performance_schema.threads b ON b.THREAD_ID=w.BLOCKING_THREAD_ID \
                     WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='seata_rs_at_branch_v1' \
                     AND l.INDEX_NAME='PRIMARY' AND l.LOCK_TYPE='RECORD' AND b.PROCESSLIST_ID=?",
            )
            .bind(blocker_id)
            .fetch_optional(&mut observer)
            .await
            .unwrap();
            if wait.is_some() || tokio::time::Instant::now() >= deadline {
                break wait;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let (first, wait) = tokio::join!(
        adapter.recover_commit_attempt(xid, branch_id),
        wait_for_lock
    );
    let error = first.expect_err("failed 004 witness query must remain unknown");
    assert!(error.is_unknown_outcome());
    // 两个服务端限时均为两秒，先触发的计时器决定原错误码。
    let mysql_errno = match error {
        MySqlCommitRecoveryError::Observation {
            source:
                DbError::Driver {
                    operation: "RM recovery identity lock",
                    code: Some(code @ (1205 | 3024)),
                    sql_state: Some(state),
                    ..
                },
        } if state == "HY000" => code,
        other => panic!("expected MySQL to time out the 004 query: {other:?}"),
    };
    let (waiting_id, waiting_thread_id) = wait.expect("missing original 004 row lock wait");
    fixture::save(
        "java-witness-query-unknown.json",
        &serde_json::json!({
            "xid": xid, "branch_id": branch_id, "first_recovery": "ObservationUnknown",
            "mysql_errno": mysql_errno, "sql_state": "HY000", "blocking_connection": blocker_id,
            "waiting_connection": waiting_id, "waiting_thread": waiting_thread_id,
        }),
    );

    // 保持锁直到独立 SQL 核对 TC 和业务状态，避免先释放再误把补做围栏算入首次失败。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !observations.join("java-witness-query-reobserve").is_file() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "independent recovery-query snapshot missing"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    blocker.rollback().await.unwrap();
}
