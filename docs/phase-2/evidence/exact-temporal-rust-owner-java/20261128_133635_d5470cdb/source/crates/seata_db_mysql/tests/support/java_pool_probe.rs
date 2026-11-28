//! 复用同一业务池，通过真实本地事务验证换租后的连接。

use crate::fixture;
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::MySqlDataSource;
use std::{path::Path, time::Duration};

/// 等待原物理会话关闭的独立快照，再借出指定数量的连接并确认回滚。
pub(super) async fn exercise(source: &MySqlDataSource, observations: &Path, xid: &str, slots: u8) {
    wait_for_snapshot(observations, "java-pool-probe-permitted").await;
    let holder = if slots == 2 {
        let mut holder = source.begin().await.unwrap();
        assert_eq!(
            holder
                .execute(
                    &SqlRecognizerFactory::recognize(
                        "UPDATE undo_work SET v='pool_holder_no_match' WHERE id=97",
                    )
                    .unwrap(),
                    &[],
                )
                .await
                .unwrap(),
            0
        );
        Some(holder)
    } else {
        None
    };
    let mut probe = source.begin().await.unwrap();
    let changed = probe
        .execute(
            &SqlRecognizerFactory::recognize(
                "UPDATE undo_work SET v='pool_probe_rolled_back' WHERE id=1",
            )
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(changed, 1);
    probe.rollback().await.unwrap();
    if let Some(holder) = holder {
        holder.rollback().await.unwrap();
    }
    fixture::save(
        "java-pool-probe.json",
        &serde_json::json!({
            "xid": xid, "pool_limit": slots, "probe_affected_rows": changed, "probe_rollback": "Confirmed",
        }),
    );
}

/// 通过运行器写入的标记等待独立数据库采样。
pub(super) async fn wait_for_snapshot(observations: &Path, marker: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !observations.join(marker).is_file() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "missing independent snapshot {marker}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
