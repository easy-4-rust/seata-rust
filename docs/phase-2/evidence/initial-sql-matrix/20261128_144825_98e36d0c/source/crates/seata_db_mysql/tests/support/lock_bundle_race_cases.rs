//! 从真实 MySQL 镜像取原锁键，在原 TC 批量 INSERT 的公共键位置同步两个分支。
use super::{fixture, lock_guard_cases::checkpoint};
use seata_at::{ConnectionContext, MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_protocol::transaction::{BranchRegisterRequest, GlobalLockQueryRequest};
use seata_protocol::{BranchType, GlobalStatus, Message, TransactionExceptionCode};
use seata_rm::{BranchResource, ResourceManager, RmCallError};
use seata_tm::{TransactionConfig, TransactionManager};
use sqlx::MySqlConnection;
use std::{path::Path, time::Duration};
use tokio::time::{sleep, Instant};

async fn lock_key(source: &MySqlDataSource, statement: &str) -> String {
    let mut tx = source.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize(statement).unwrap();
    assert_eq!(tx.execute(&sql, &[]).await.unwrap(), 2);
    let key = ConnectionContext::for_phase_one_registration(
        tx.undo_logs(),
        MySqlJdbcProfile::Connector8Default,
    )
    .unwrap()
    .build_lock_keys()
    .unwrap();
    tx.rollback().await.unwrap();
    key
}

/// 使用运行器自有数据库、数据源和原 TC 会话验证锁批次竞态。
/// 在等待目录保留独立 SQL 检查点，任一登记或清理结果不符时使测试失败。
pub(super) async fn exercise(
    admin: &mut MySqlConnection,
    source: &MySqlDataSource,
    manager: &MySqlDataSourceManager,
    rm: &ResourceManager,
    tm: &TransactionManager,
    folder: &Path,
) {
    // 先回滚业务行锁，使两个请求在 TC 锁表中竞争。
    let a_key = lock_key(
        source,
        "UPDATE accounts SET balance=balance-10 WHERE id IN (1,3)",
    )
    .await;
    let b_key = lock_key(
        source,
        "UPDATE accounts SET balance=balance-10 WHERE id IN (2,3)",
    )
    .await;
    assert_eq!((&*a_key, &*b_key), ("accounts:1,3", "accounts:2,3"));
    let db: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    assert!(db.starts_with("undo_") && db.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
    let gate = format!("{db}_tc_insert");
    // 独有键写入后才标记线程，在公共键写入前汇合。
    sqlx::raw_sql(
        "CREATE TRIGGER fixture_private_lock_inserted AFTER INSERT ON lock_table FOR EACH ROW \
         BEGIN IF NEW.pk IN ('1','2') THEN SET @seata_fixture_private_key=NEW.pk; END IF; END",
    )
    .execute(&mut *admin)
    .await
    .unwrap();
    sqlx::raw_sql(&format!(
        "CREATE TRIGGER fixture_overlap_lock_gate BEFORE INSERT ON lock_table FOR EACH ROW \
         BEGIN IF NEW.pk='3' THEN \
         IF COALESCE(GET_LOCK('{gate}',15),0)<>1 THEN \
         SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='owned overlap lock gate timed out'; \
         END IF; DO RELEASE_LOCK('{gate}'); END IF; END",
    ))
    .execute(&mut *admin)
    .await
    .unwrap();
    let held: i64 = sqlx::query_scalar("SELECT GET_LOCK(?,0)")
        .bind(&gate)
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    assert_eq!(held, 1);

    let cfg = TransactionConfig::builder("tc-lock-compare-and-insert".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut a = tm.begin(&cfg).await.unwrap();
    let mut b = tm.begin(&cfg).await.unwrap();
    let a_xid = a.xid().clone();
    let b_xid = b.xid().clone();
    let resource = manager.key().resource_id();
    let request = |xid: &str, key: String| BranchRegisterRequest {
        xid: Some(xid.into()),
        branch_type: BranchType::At,
        resource_id: Some(resource.into()),
        application_data: None,
        lock_key: Some(key),
    };
    let client = rm.client();
    let a_register = client.branch_register_with_receipt(request(a_xid.as_str(), a_key));
    let b_register = client.branch_register_with_receipt(request(b_xid.as_str(), b_key));
    let watch = async {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let private_keys: Vec<String> = sqlx::query_scalar(
                "SELECT CAST(v.VARIABLE_VALUE AS CHAR CHARACTER SET utf8mb4) \
                 FROM performance_schema.metadata_locks AS m \
                 JOIN performance_schema.user_variables_by_thread AS v ON v.THREAD_ID=m.OWNER_THREAD_ID \
                 WHERE m.OBJECT_TYPE='USER LEVEL LOCK' AND m.OBJECT_NAME=? AND m.LOCK_STATUS='PENDING' \
                 AND v.VARIABLE_NAME='seata_fixture_private_key' ORDER BY v.VARIABLE_VALUE",
            ).bind(&gate).fetch_all(&mut *admin).await.unwrap();
            if private_keys == ["1", "2"] {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "both original TC workers must reach INSERT with a private key"
            );
            sleep(Duration::from_millis(25)).await;
        }
        checkpoint(
            folder,
            "both-lock-inserts-pending",
            &serde_json::json!({
                "gate":gate,"xid_a":a_xid.as_str(),"xid_b":b_xid.as_str(),"resource":resource
            }),
        )
        .await;
        let released: i64 = sqlx::query_scalar("SELECT RELEASE_LOCK(?)")
            .bind(&gate)
            .fetch_one(&mut *admin)
            .await
            .unwrap();
        assert_eq!(released, 1);
    };
    let (result_a, result_b, ()) = tokio::join!(a_register, b_register, watch);
    let (receipt, error, private_key, loser) = match (result_a, result_b) {
        (Ok(receipt), Err(error)) => (receipt, error, "1", b_xid.as_str()),
        (Err(error), Ok(receipt)) => (receipt, error, "2", a_xid.as_str()),
        results => panic!("expected one complete lock bundle: {results:?}"),
    };
    let RmCallError::DefiniteRejected { response, .. } = error else {
        panic!("expected an original definite rejection: {error:?}");
    };
    let Some(Message::BranchRegisterResponse(reply)) = response.body else {
        panic!("expected original BranchRegisterResponse");
    };
    assert_eq!(
        reply.response.transaction_exception_code,
        TransactionExceptionCode::LockKeyConflict
    );
    let losing_private = if private_key == "1" {
        "accounts:2"
    } else {
        "accounts:1"
    };
    assert!(
        client
            .lock_query(GlobalLockQueryRequest {
                xid: Some(loser.into()),
                branch_type: BranchType::At,
                resource_id: Some(resource.into()),
                lock_key: Some(losing_private.into()),
                application_data: None,
            })
            .await
            .unwrap()
            .lockable
    );
    let winner = receipt.identity();
    manager.enroll(&receipt).await.unwrap();
    let result = serde_json::json!({
        "winner":winner.xid(),"branch_id":winner.branch_id(),"loser":loser,"resource":resource,
        "private_key":private_key,"lock_conflict_code":reply.response.transaction_exception_code.to_java_v1()
    });
    checkpoint(folder, "one-lock-bundle-won", &result).await;
    a.rollback().await.unwrap();
    b.rollback().await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if tm.get_status(&a_xid).await.unwrap().status() == GlobalStatus::Finished
            && tm.get_status(&b_xid).await.unwrap().status() == GlobalStatus::Finished
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "original race XIDs remain active"
        );
        sleep(Duration::from_millis(40)).await;
    }
    fixture::save("tc-lock-guard-result.json", &result);
}
