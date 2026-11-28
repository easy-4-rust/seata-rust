//! 自有真实 MySQL 的原生两阶段、失败清理和跨进程重启；不冒充原 TC XA Gate。
#![cfg(feature = "xa")]
#![allow(clippy::unwrap_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/xa_command_proxy.rs"]
mod xa_command_proxy;
use seata_at::SqlRecognizerFactory;
use seata_datasource::FieldValue;
use seata_db_mysql::{DbError, MySqlDataSource, MySqlXaDataSource, MySqlXaError};
use seata_xa::{XaBranchXid, XaOperation};
use std::time::Duration;

fn xid(name: &str, branch: i64) -> XaBranchXid {
    XaBranchXid::new(Some(format!("xa:native:{name}")), branch)
}
async fn source() -> (sqlx::MySqlConnection, MySqlDataSource, MySqlXaDataSource) {
    let admin = owned_mysql::admin().await;
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let xa = MySqlXaDataSource::new(source.clone());
    (admin, source, xa)
}
async fn seed(admin: &mut sqlx::MySqlConnection) {
    sqlx::raw_sql("DROP TABLE IF EXISTS xa_core")
        .execute(&mut *admin)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE xa_core(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB")
        .execute(&mut *admin)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO xa_core VALUES(1,10)")
        .execute(admin)
        .await
        .unwrap();
}
async fn value(admin: &mut sqlx::MySqlConnection, id: i32) -> Option<i32> {
    sqlx::query_scalar("SELECT v FROM xa_core WHERE id=?")
        .bind(id)
        .fetch_optional(admin)
        .await
        .unwrap()
}
async fn cleared(admin: &mut sqlx::MySqlConnection, id: i32) {
    // 先确认原服务器事务已经消失，再检查行；不能用未提交行不可见冒充回滚。
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let active: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.innodb_trx")
                    .fetch_one(&mut *admin)
                    .await
                    .unwrap();
            if active == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(value(admin, id).await, None);
}
fn record(name: &str, mut value: serde_json::Value) {
    value.as_object_mut().unwrap().insert(
        "rust_process_id".into(),
        serde_json::json!(std::process::id()),
    );
    let folder = std::path::PathBuf::from(std::env::var_os("SEATA_XA_OUT").unwrap());
    assert!(
        folder.is_absolute()
            && (folder.to_string_lossy().starts_with("D:")
                || folder.to_string_lossy().starts_with("E:"))
    );
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL XA runtime"]
async fn prepared_is_invisible_locked_and_resolved_only_by_original_identity() {
    let (mut admin, source, xa) = source().await;
    seed(&mut admin).await;
    let commit = xid("two-phase", 41);
    let rollback = xid("two-phase", 42);
    let mut first = xa.start(commit.clone()).await.unwrap();
    assert_eq!(
        first
            .execute(
                &SqlRecognizerFactory::recognize("UPDATE xa_core SET v=? WHERE id=1").unwrap(),
                &[FieldValue::Signed(11)]
            )
            .await
            .unwrap(),
        1
    );
    assert_eq!(first.prepare().await.unwrap(), commit);
    assert!(
        xa.prepared_observed(&commit).await.unwrap(),
        "original Prepared branch missing"
    );
    assert!(
        !xa.prepared_observed(&rollback).await.unwrap(),
        "other branch qualifier was mistaken for Prepared"
    );
    assert_eq!(
        value(&mut admin, 1).await,
        Some(10),
        "XA PREPARE exposed uncommitted business data"
    );
    sqlx::raw_sql("SET SESSION innodb_lock_wait_timeout=1")
        .execute(&mut admin)
        .await
        .unwrap();
    let lock = sqlx::raw_sql("UPDATE xa_core SET v=99 WHERE id=1")
        .execute(&mut admin)
        .await
        .unwrap_err();
    let lock_code = lock
        .as_database_error()
        .unwrap()
        .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
        .unwrap()
        .number();
    assert_eq!(
        lock_code, 1205,
        "Prepared failed to retain original row lock"
    );
    xa.commit_prepared(&commit).await.unwrap();
    assert_eq!(
        value(&mut admin, 1).await,
        Some(11),
        "original XA COMMIT must publish original business row"
    );
    assert!(!xa.prepared_observed(&commit).await.unwrap());
    let error = xa.commit_prepared(&commit).await.unwrap_err();
    assert!(
        matches!(
            error,
            MySqlXaError::Unknown {
                operation: XaOperation::Commit,
                source: DbError::Driver {
                    code: Some(1397),
                    ..
                }
            }
        ),
        "XAER_NOTA must not become a successful commit"
    );
    let mut second = xa.start(rollback.clone()).await.unwrap();
    second
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_core(id,v) VALUES(2,20)").unwrap(),
            &[],
        )
        .await
        .unwrap();
    second.prepare().await.unwrap();
    assert!(xa.prepared_observed(&rollback).await.unwrap());
    assert_eq!(value(&mut admin, 2).await, None);
    xa.rollback_prepared(&rollback).await.unwrap();
    assert!(!xa.prepared_observed(&rollback).await.unwrap());
    assert_eq!(value(&mut admin, 2).await, None);
    sqlx::raw_sql("UPDATE xa_core SET v=11 WHERE id=1")
        .execute(&mut admin)
        .await
        .unwrap();
    record(
        "two-phase",
        serde_json::json!({"commit_branch":41,"rollback_branch":42,"prepared_invisible":true,"prepared_lock_error":lock_code,"committed_value":value(&mut admin,1).await,"rollback_row":value(&mut admin,2).await,"repeat_commit":"Unknown:1397","all_prepared_cleared":true}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL XA runtime"]
async fn original_hex_identity_and_active_rollback_preserve_values() {
    let (mut admin, source, xa) = source().await;
    seed(&mut admin).await;
    let identity = XaBranchXid::new(Some("xa:';--\\\n中".into()), 71);
    let foreign = format!(
        "{},999",
        identity
            .mysql_arguments()
            .unwrap()
            .strip_suffix(",9752")
            .unwrap()
    );
    for command in [
        format!("XA START {foreign}"),
        "INSERT INTO xa_core(id,v) VALUES(6,60)".to_owned(),
        format!("XA END {foreign}"),
        format!("XA PREPARE {foreign}"),
    ] {
        sqlx::raw_sql(&command).execute(&mut admin).await.unwrap();
    }
    let foreign_rows = sqlx::raw_sql("XA RECOVER")
        .fetch_all(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        foreign_rows.len(),
        1,
        "foreign Prepared test fixture is missing"
    );
    assert!(
        !xa.prepared_observed(&identity).await.unwrap(),
        "foreign XA format was mistaken for Seata Prepared"
    );
    sqlx::raw_sql(&format!("XA ROLLBACK {foreign}"))
        .execute(&mut admin)
        .await
        .unwrap();
    let mut tx = xa.start(identity.clone()).await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("INSERT INTO xa_core(id,v) VALUES(7,70)").unwrap(),
        &[],
    )
    .await
    .unwrap();
    tx.prepare().await.unwrap();
    assert!(xa.prepared_observed(&identity).await.unwrap());
    xa.commit_prepared(&identity).await.unwrap();
    assert_eq!(value(&mut admin, 7).await, Some(70));
    let mut active = xa.start(xid("active-rollback", 72)).await.unwrap();
    active
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_core(id,v) VALUES(8,80)").unwrap(),
            &[],
        )
        .await
        .unwrap();
    active.rollback().await.unwrap();
    assert_eq!(value(&mut admin, 8).await, None);
    record(
        "hex-and-active-rollback",
        serde_json::json!({"hex_identity_committed":true,"value":70,"active_rollback_confirmed":true}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL XA runtime"]
async fn failed_or_cancelled_dml_poison_scope_and_close_original_session() {
    let (mut admin, source, xa) = source().await;
    seed(&mut admin).await;
    let mut dropped = xa.start(xid("dropped", 80)).await.unwrap();
    dropped
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_core(id,v) VALUES(9,90)").unwrap(),
            &[],
        )
        .await
        .unwrap();
    drop(dropped);
    cleared(&mut admin, 9).await;
    let mut failed = xa.start(xid("failed", 81)).await.unwrap();
    failed
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_core(id,v) VALUES(8,80)").unwrap(),
            &[],
        )
        .await
        .unwrap();
    assert!(failed
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_core(id,v) VALUES(1,80)").unwrap(),
            &[]
        )
        .await
        .is_err());
    assert!(matches!(
        failed
            .execute(
                &SqlRecognizerFactory::recognize("DELETE FROM xa_core WHERE id=1").unwrap(),
                &[]
            )
            .await,
        Err(MySqlXaError::Poisoned)
    ));
    assert!(matches!(
        failed.prepare().await,
        Err(MySqlXaError::Poisoned)
    ));
    cleared(&mut admin, 8).await;
    let mut locker = owned_mysql::admin().await;
    sqlx::raw_sql("START TRANSACTION")
        .execute(&mut locker)
        .await
        .unwrap();
    sqlx::raw_sql("UPDATE xa_core SET v=12 WHERE id=1")
        .execute(&mut locker)
        .await
        .unwrap();
    let mut cancelled = xa.start(xid("cancelled", 82)).await.unwrap();
    let update = SqlRecognizerFactory::recognize("UPDATE xa_core SET v=13 WHERE id=1").unwrap();
    let blocker_connection_id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
        .fetch_one(&mut locker)
        .await
        .unwrap();
    let mut pending = Box::pin(cancelled.execute(&update, &[]));
    let observed = async {
        loop {
            let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.threads b ON b.THREAD_ID=w.BLOCKING_THREAD_ID WHERE b.PROCESSLIST_ID=?").bind(blocker_connection_id).fetch_one(&mut admin).await.unwrap();
            if count > 0 {
                break count;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    };
    let actual_lock_waits_before_cancel = tokio::select! {result=&mut pending=>panic!("XA DML completed before authoritative lock-wait observation: {result:?}"),count=tokio::time::timeout(Duration::from_millis(1500),observed)=>count.unwrap()};
    drop(pending);
    assert!(matches!(
        cancelled.prepare().await,
        Err(MySqlXaError::Poisoned)
    ));
    sqlx::raw_sql("ROLLBACK")
        .execute(&mut locker)
        .await
        .unwrap();
    cleared(&mut admin, 8).await;
    assert_eq!(value(&mut admin, 1).await, Some(10));
    assert!(!xa.prepared_observed(&xid("cancelled", 82)).await.unwrap());
    let mut invalid = xa.start(xid("parameter", 83)).await.unwrap();
    assert!(matches!(
        invalid.execute(&update, &[FieldValue::Signed(1)]).await,
        Err(MySqlXaError::NotSent {
            operation: XaOperation::Execute,
            ..
        })
    ));
    assert!(matches!(
        invalid.prepare().await,
        Err(MySqlXaError::Poisoned)
    ));
    record(
        "poison-and-cancel",
        serde_json::json!({"duplicate_key_poisoned":true,"cancelled_lock_wait_poisoned":true,"actual_lock_waits_before_cancel":actual_lock_waits_before_cancel,"blocker_connection_id":blocker_connection_id,"parameter_rejection_poisoned":true,"original_value":10,"partial_insert_absent":true,"prepared_absent_not_terminal_proof":true}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL XA runtime"]
async fn unsafe_tables_are_rejected_before_dml() {
    let (mut admin, source, xa) = source().await;
    seed(&mut admin).await;
    sqlx::raw_sql("DROP TABLE IF EXISTS xa_unsafe")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE xa_unsafe(id INT PRIMARY KEY) ENGINE=MyISAM")
        .execute(&mut admin)
        .await
        .unwrap();
    let mut unsafe_tx = xa.start(xid("unsafe", 91)).await.unwrap();
    assert!(
        matches!(
            unsafe_tx
                .execute(
                    &SqlRecognizerFactory::recognize("INSERT INTO xa_unsafe(id) VALUES(1)")
                        .unwrap(),
                    &[]
                )
                .await,
            Err(MySqlXaError::NotSent { .. })
        ),
        "non-InnoDB XA write was not rejected before DML"
    );
    drop(unsafe_tx);
    sqlx::raw_sql(
        "CREATE TRIGGER xa_core_trigger BEFORE UPDATE ON xa_core FOR EACH ROW SET NEW.v=123",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    let mut trigger_tx = xa.start(xid("trigger", 92)).await.unwrap();
    assert!(
        matches!(
            trigger_tx
                .execute(
                    &SqlRecognizerFactory::recognize("UPDATE xa_core SET v=99 WHERE id=1").unwrap(),
                    &[]
                )
                .await,
            Err(MySqlXaError::NotSent { .. })
        ),
        "XA trigger was not rejected before DML"
    );
    drop(trigger_tx);
    sqlx::raw_sql("DROP TRIGGER xa_core_trigger")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql("DROP TABLE IF EXISTS xa_parent")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE xa_parent(id INT PRIMARY KEY) ENGINE=InnoDB")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO xa_parent VALUES(10)")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql(
        "ALTER TABLE xa_core ADD CONSTRAINT xa_core_fk FOREIGN KEY(v) REFERENCES xa_parent(id)",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    let mut foreign_tx = xa.start(xid("foreign-key", 94)).await.unwrap();
    assert!(
        matches!(
            foreign_tx
                .execute(
                    &SqlRecognizerFactory::recognize("UPDATE xa_core SET v=10 WHERE id=1").unwrap(),
                    &[]
                )
                .await,
            Err(MySqlXaError::NotSent { .. })
        ),
        "XA foreign key was not rejected before DML"
    );
    drop(foreign_tx);
    sqlx::raw_sql("ALTER TABLE xa_core DROP FOREIGN KEY xa_core_fk")
        .execute(&mut admin)
        .await
        .unwrap();
    let mut reserved = xa.start(xid("reserved", 93)).await.unwrap();
    assert!(matches!(
        reserved
            .execute(
                &SqlRecognizerFactory::recognize("DELETE FROM undo_log WHERE id=1").unwrap(),
                &[]
            )
            .await,
        Err(MySqlXaError::NotSent { .. })
    ));
    drop(reserved);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM xa_unsafe")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    assert_eq!(value(&mut admin, 1).await, Some(10));
    record(
        "unsafe-tables",
        serde_json::json!({"myisam_rejected_before_dml":true,"trigger_rejected_before_dml":true,"foreign_key_rejected_before_dml":true,"internal_table_rejected_before_dml":true,"myisam_rows":rows,"business_value":10}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "runner stages real process exit and MySQL restart"]
async fn prepare_original_branches_before_external_restart() {
    let (mut admin, source, xa) = source().await;
    seed(&mut admin).await;
    for (branch, id) in [(101, 10), (102, 11)] {
        let original = xid("restart", branch);
        let mut tx = xa.start(original.clone()).await.unwrap();
        tx.execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_core(id,v) VALUES(?,?)").unwrap(),
            &[FieldValue::Signed(id), FieldValue::Signed(id * 10)],
        )
        .await
        .unwrap();
        tx.prepare().await.unwrap();
        assert!(xa.prepared_observed(&original).await.unwrap());
        assert_eq!(value(&mut admin, id as i32).await, None);
    }
    let server: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    record(
        "before-restart",
        serde_json::json!({"server_uuid":server,"commit_branch":101,"rollback_branch":102,"prepared":2,"both_business_rows_invisible":true}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "runner stages real process exit and MySQL restart"]
async fn recover_original_branches_after_external_restart() {
    let (mut admin, source, xa) = source().await;
    for branch in [101, 102] {
        assert!(
            xa.prepared_observed(&xid("restart", branch)).await.unwrap(),
            "restarted MySQL lost original Prepared branch"
        );
    }
    xa.commit_prepared(&xid("restart", 101)).await.unwrap();
    xa.rollback_prepared(&xid("restart", 102)).await.unwrap();
    assert_eq!(value(&mut admin, 10).await, Some(100));
    assert_eq!(value(&mut admin, 11).await, None);
    let rows = sqlx::raw_sql("XA RECOVER")
        .fetch_all(&mut admin)
        .await
        .unwrap();
    assert!(rows.is_empty());
    let server: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    record(
        "after-restart",
        serde_json::json!({"server_uuid":server,"committed_value":100,"rollback_row":null,"prepared_remaining":rows.len()}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL XA runtime"]
async fn detached_preparation_configuration_is_required() {
    let (mut admin, source, xa) = source().await;
    sqlx::raw_sql("SET GLOBAL xa_detach_on_prepare=OFF")
        .execute(&mut admin)
        .await
        .unwrap();
    let result = xa.start(xid("detached-disabled", 111)).await;
    // 夹具先恢复全局配置，再对真实入口的拒绝作断言。
    sqlx::raw_sql("SET GLOBAL xa_detach_on_prepare=ON")
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(
        matches!(
            result,
            Err(MySqlXaError::NotSent {
                operation: XaOperation::Start,
                ..
            })
        ),
        "XA admitted a non-detached session"
    );
    record(
        "detached-required",
        serde_json::json!({"detached_off_rejected_before_start":true,"global_config_restored":true}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL with original XA OK interception"]
async fn original_xa_ok_loss_preserves_unknown_without_automatic_replay() {
    let (mut admin, plain_source, plain) = source().await;
    seed(&mut admin).await;
    let mut proofs = Vec::new();
    for (offset, operation) in [
        XaOperation::Prepare,
        XaOperation::Commit,
        XaOperation::Rollback,
    ]
    .into_iter()
    .enumerate()
    {
        let branch = 201 + offset as i64;
        let id = 21 + offset as i64;
        let original = xid("ok-loss", branch);
        let verb = match operation {
            XaOperation::Prepare => "PREPARE",
            XaOperation::Commit => "COMMIT",
            _ => "ROLLBACK",
        };
        let mut command = vec![3];
        command.extend_from_slice(
            format!("XA {verb} {}", original.mysql_arguments().unwrap()).as_bytes(),
        );
        let proxy = xa_command_proxy::XaCommandProxy::start(
            std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
            command.clone(),
        )
        .await;
        let proxied_source = MySqlDataSource::connect(
            owned_mysql::options(Some(proxy.port))
                .operation_timeout(Duration::from_millis(750))
                .unwrap(),
        )
        .await
        .unwrap();
        let proxied = MySqlXaDataSource::new(proxied_source.clone());
        let mut tx = proxied.start(original.clone()).await.unwrap();
        tx.execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_core(id,v) VALUES(?,?)").unwrap(),
            &[FieldValue::Signed(id), FieldValue::Signed(id * 10)],
        )
        .await
        .unwrap();
        let error = if operation == XaOperation::Prepare {
            proxy.arm();
            tx.prepare().await.unwrap_err()
        } else {
            tx.prepare().await.unwrap();
            proxy.arm();
            if operation == XaOperation::Commit {
                proxied.commit_prepared(&original).await.unwrap_err()
            } else {
                proxied.rollback_prepared(&original).await.unwrap_err()
            }
        };
        assert!(
            matches!(&error,MySqlXaError::Unknown{operation:actual,..} if *actual==operation),
            "original XA OK loss must preserve unknown"
        );
        let (request, response) = proxy.captured().unwrap();
        assert_eq!(request[4..], command);
        assert_eq!(response[3], 1);
        assert_eq!(response[4], 0, "relay did not intercept actual XA success");
        let prepared = plain.prepared_observed(&original).await.unwrap();
        assert_eq!(prepared, operation == XaOperation::Prepare);
        let observed = value(&mut admin, id as i32).await;
        assert_eq!(
            observed,
            if operation == XaOperation::Commit {
                Some((id * 10) as i32)
            } else {
                None
            }
        );
        if prepared {
            plain.rollback_prepared(&original).await.unwrap();
        }
        let actual_commands = proxy.shutdown().await;
        assert_eq!(
            actual_commands, 1,
            "original XA command was automatically replayed"
        );
        proofs.push(serde_json::json!({"operation":verb,"branch_id":branch,"request":request,"held_original_ok":response,"result":"Unknown","prepared_observed":prepared,"business_value":observed,"actual_commands":actual_commands}));
        proxied_source.close().await.unwrap();
    }
    record(
        "original-xa-ok-loss",
        serde_json::json!({"cases":proofs,"automatic_replay":false}),
    );
    plain_source.close().await.unwrap();
}
