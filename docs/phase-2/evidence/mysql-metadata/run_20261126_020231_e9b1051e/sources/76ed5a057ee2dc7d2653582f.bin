//! 固定 Java TC 下同一复合主键的交叠 UPDATE、DELETE 和 INSERT。

use super::fixture;
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::MySqlLocalTransaction;
use seata_protocol::GlobalStatus;
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, FileBranchStore};
use seata_tm::{GlobalTransaction, TransactionCompletion, TransactionManager};
use sqlx::MySqlConnection;
use std::time::Duration;
use tokio::time::Instant;

const KEY_A: i64 = 9_007_199_254_740_993;
const KEY_B: i64 = 9_007_199_254_740_994;
type BusinessRow = (i64, i64, Option<String>);

/// 只在自有测试库中建立复合主键表及两个相邻的隔离行。
pub(super) async fn seed_table(admin: &mut MySqlConnection) {
    sqlx::raw_sql(
        "DROP TABLE undo_work;
         CREATE TABLE undo_work(id BIGINT NOT NULL, segment BIGINT NOT NULL,
             v VARCHAR(1000), PRIMARY KEY (id,segment)) ENGINE=InnoDB;
         INSERT INTO undo_work VALUES
             (9007199254740993,1,'original'),
             (9007199254740993,2,NULL),
             (9007199254740994,1,'neighbor');",
    )
    .execute(admin)
    .await
    .unwrap();
}

/// 通过正式 AT 入口运行顺序敏感的四条业务语句。
pub(super) async fn execute_mixed(local: &mut MySqlLocalTransaction) {
    for statement in [
        "UPDATE undo_work SET v='middle' WHERE id=9007199254740993 AND segment=1",
        "DELETE FROM undo_work WHERE id=9007199254740993 AND segment=1",
        "INSERT INTO undo_work(id,segment,v) VALUES(9007199254740993,1,'replacement')",
        "INSERT INTO undo_work(id,segment,v) VALUES(9007199254740994,2,'transient')",
    ] {
        assert_eq!(
            local
                .execute(&SqlRecognizerFactory::recognize(statement).unwrap(), &[])
                .await
                .unwrap(),
            1
        );
    }
}

async fn rows(admin: &mut MySqlConnection) -> Vec<BusinessRow> {
    sqlx::query_as("SELECT id,segment,v FROM undo_work ORDER BY id,segment")
        .fetch_all(admin)
        .await
        .unwrap()
}

/// 完成原全局事务并核对业务行、原锁身份和持久 RM 记录。
pub(super) async fn exercise(
    admin: &mut MySqlConnection,
    global: &mut GlobalTransaction<'_>,
    tm: &TransactionManager,
    store: &FileBranchStore,
    identity: &BranchIdentity,
    phase: BranchPhase,
    lock_key: &str,
) {
    let before = rows(admin).await;
    assert_eq!(
        before,
        vec![
            (KEY_A, 1, Some("replacement".into())),
            (KEY_A, 2, None),
            (KEY_B, 1, Some("neighbor".into())),
            (KEY_B, 2, Some("transient".into()))
        ]
    );
    let mut pks: Vec<String> =
        sqlx::query_scalar("SELECT pk FROM lock_table WHERE xid=? AND branch_id=?")
            .bind(identity.xid())
            .bind(identity.branch_id())
            .fetch_all(&mut *admin)
            .await
            .unwrap();
    let mut registered: Vec<_> = lock_key
        .split(';')
        .flat_map(|group| group.strip_prefix("undo_work:").unwrap().split(','))
        .collect();
    registered.sort_unstable();
    pks.sort_unstable();
    assert_eq!(pks, registered);

    let (completion, expected_status) = match phase {
        BranchPhase::Commit => (global.commit().await.unwrap(), GlobalStatus::Committed),
        BranchPhase::Rollback => (global.rollback().await.unwrap(), GlobalStatus::Rollbacked),
    };
    assert!(
        matches!(completion, TransactionCompletion::Observed { outcome, .. }
        if outcome.status() == expected_status)
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    let xid = seata_tm::Xid::parse(identity.xid().into()).unwrap();
    while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
        assert!(Instant::now() < deadline, "original Java TC did not finish");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert_eq!(
        store.lease(identity.clone(), phase).await.unwrap().state(),
        Some(BranchRecordState::Completed)
    );
    let after = rows(admin).await;
    if phase == BranchPhase::Commit {
        assert_eq!(after, before);
    } else {
        assert_eq!(
            after,
            vec![
                (KEY_A, 1, Some("original".into())),
                (KEY_A, 2, None),
                (KEY_B, 1, Some("neighbor".into()))
            ]
        );
    }
    fixture::save(
        "java-undo-mixed.json",
        &serde_json::json!({
            "xid": identity.xid(), "branch_id": identity.branch_id(),
            "resource_id": identity.resource().resource_id(), "lock_key": lock_key, "physical_pks": pks,
            "before": before, "after": after, "end_status": expected_status.to_java_v1()
        }),
    );
}
