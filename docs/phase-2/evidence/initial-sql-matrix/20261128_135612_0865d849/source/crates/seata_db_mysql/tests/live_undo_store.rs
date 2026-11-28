//! 真实同事务Undo写入；独立脏读/已提交读和InnoDB事务号，不能用客户端返回值替代原子性证据。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{DbError, MySqlDataSource};
use sqlx::{Connection, Row};

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn undo_current_schema_rejects_late_constraints_columns_and_identity_index_changes() {
    for (change,expected) in [
        ("CREATE TRIGGER late_undo_log AFTER INSERT ON undo_log FOR EACH ROW INSERT INTO undo_side_effect VALUES(NEW.branch_id)","Undo trigger or foreign key"),
        ("ALTER TABLE undo_log DROP INDEX ux_undo_log","Undo original unique identity"),
        ("ALTER TABLE undo_log MODIFY xid VARCHAR(129) NOT NULL","Undo text capacity or charset"),
        ("ALTER TABLE undo_log ADD COLUMN hidden_value INT NULL INVISIBLE","Undo seven-column shape"),
        ("ALTER TABLE undo_log MODIFY log_status INT UNSIGNED NOT NULL","Undo original column shape"),
        ("ALTER TABLE undo_log ADD CONSTRAINT late_undo_fk FOREIGN KEY(xid) REFERENCES undo_parent(xid)","Undo trigger or foreign key"),
        ("CREATE TABLE undo_child(xid VARCHAR(128) NOT NULL,branch_id BIGINT NOT NULL,FOREIGN KEY(xid,branch_id) REFERENCES undo_log(xid,branch_id)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4","Undo trigger or foreign key"),
    ] {
        let mut observer=fixture::reset().await;
        sqlx::raw_sql("DROP TABLE IF EXISTS undo_side_effect,undo_parent;
            CREATE TABLE undo_side_effect(id BIGINT) ENGINE=MyISAM;
            CREATE TABLE undo_parent(xid VARCHAR(128) PRIMARY KEY) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4")
            .execute(&mut observer).await.unwrap();
        let ds=MySqlDataSource::connect(owned_mysql::options(None)).await.unwrap();
        let mut tx=ds.begin().await.unwrap();
        tx.execute(&SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'once')").unwrap(),&[])
            .await.unwrap();
        sqlx::raw_sql(change).execute(&mut observer).await.unwrap();
        let result=tx.flush_undo_logs("undo-store:xid",42,MySqlJdbcProfile::Connector8Default).await;
        assert!(matches!(result,Err(DbError::Unsupported { feature }) if feature == expected),
            "late Undo definition was accepted or misclassified: {change}, {result:?}");
        assert!(matches!(tx.table_meta("undo_work").await,Err(DbError::Poisoned)));
        tx.rollback().await.unwrap();
        assert_eq!(fixture::count(&mut observer,"undo_work").await,0);
        assert_eq!(fixture::count(&mut observer,"undo_log").await,0);
        assert_eq!(fixture::count(&mut observer,"undo_side_effect").await,0);
        println!("late Undo schema rejected without writes or side effects: {change}");
        ds.close().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn undo_engine_change_after_business_metadata_snapshot_is_rejected_before_flush() {
    for (case,change) in [
        "ALTER TABLE undo_log ENGINE=MyISAM",
        "ALTER TABLE undo_log ENGINE=MyISAM, MODIFY xid VARCHAR(128) NOT NULL COMMENT 'fake\n) ENGINE=InnoDB '",
        "ALTER TABLE undo_log ENGINE=MyISAM, COMMENT='fake\n) ENGINE=InnoDB '",
    ].into_iter().enumerate() {
    let mut observer=fixture::reset().await;
    let ds=MySqlDataSource::connect(owned_mysql::options(None)).await.unwrap();
    let mut tx=ds.begin().await.unwrap();
    tx.execute(&SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'once')").unwrap(),&[])
        .await.unwrap();
    sqlx::raw_sql(change).execute(&mut observer).await.unwrap();
        sqlx::query("SET SESSION sql_mode='STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION,NO_BACKSLASH_ESCAPES'")
            .execute(&mut observer).await.unwrap();
        let definition=sqlx::query("SHOW CREATE TABLE undo_log").fetch_one(&mut observer).await.unwrap();
        fixture::save(&format!("ddl-current-definition-{case}.json"),
            &serde_json::json!({"bytes":definition.get::<Vec<u8>,_>(1)}));
    let result=tx.flush_undo_logs("undo-store:xid",42,MySqlJdbcProfile::Connector8Default).await;
    assert!(matches!(result,Err(DbError::Unsupported { feature: "current physical engine must be InnoDB" })),
        "nontransactional Undo was accepted after old dictionary snapshot: {result:?}");
    assert!(matches!(tx.table_meta("undo_work").await,Err(DbError::Poisoned)));
    tx.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut observer,"undo_work").await,0);
    assert_eq!(fixture::count(&mut observer,"undo_log").await,0);
    sqlx::raw_sql("ALTER TABLE undo_log ENGINE=InnoDB").execute(&mut observer).await.unwrap();
    let mut fresh=ds.begin().await.unwrap();
    fresh.execute(&SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'fresh')").unwrap(),&[])
        .await.unwrap();
    fresh.flush_undo_logs("undo-store:xid",42,MySqlJdbcProfile::Connector8Default).await.unwrap();
    fresh.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut observer,"undo_work").await,0);
    assert_eq!(fixture::count(&mut observer,"undo_log").await,0);
    println!("old snapshot engine rejected and fresh InnoDB flush rolled back: {change:?}");
    ds.close().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn original_undo_row_and_business_share_one_uncommitted_transaction_then_rollback() {
    let mut observer = fixture::reset().await;
    sqlx::query("CREATE TABLE undo_business(id INT PRIMARY KEY,v VARCHAR(100)) ENGINE=InnoDB")
        .execute(&mut observer)
        .await
        .unwrap();
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("INSERT INTO undo_business(id,v) VALUES(1,'once')")
            .unwrap(),
        &[],
    )
    .await
    .unwrap();
    tx.flush_undo_logs("undo-store:xid", 42, MySqlJdbcProfile::Connector8Default)
        .await
        .unwrap();
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL READ UNCOMMITTED")
        .execute(&mut observer)
        .await
        .unwrap();
    let rows=sqlx::query("SELECT branch_id,xid,context,rollback_info,log_status,log_created=log_modified AS same_time FROM undo_log").fetch_all(&mut observer).await.unwrap();
    assert_eq!(
        rows.len(),
        1,
        "Undo must actually be written before flush returns"
    );
    let row = &rows[0];
    fixture::snapshot(&mut observer, "normal.json", 0).await;
    assert_eq!(row.get::<i64, _>("branch_id"), 42);
    assert_eq!(row.get::<String, _>("xid"), "undo-store:xid");
    assert_eq!(row.get::<i32, _>("log_status"), 0);
    assert_eq!(row.get::<i32, _>("same_time"), 1);
    let bytes: Vec<u8> = row.get("rollback_info");
    let log: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(log["branchId"], 42);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM undo_business")
            .fetch_one(&mut observer)
            .await
            .unwrap(),
        1
    );
    let locks=sqlx::query("SELECT OBJECT_NAME,CAST(ENGINE_TRANSACTION_ID AS CHAR) AS tid FROM performance_schema.data_locks WHERE OBJECT_SCHEMA=DATABASE() AND OBJECT_NAME IN ('undo_business','undo_log') AND LOCK_TYPE='TABLE'").fetch_all(&mut observer).await.unwrap();
    assert_eq!(locks.len(), 2);
    assert_eq!(
        locks[0].get::<String, _>("tid"),
        locks[1].get::<String, _>("tid")
    );
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut observer)
        .await
        .unwrap();
    for table in ["undo_business", "undo_log"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&mut observer)
                .await
                .unwrap(),
            0,
            "flush cannot implicitly commit either table"
        );
    }
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL READ UNCOMMITTED")
        .execute(&mut observer)
        .await
        .unwrap();
    for table in ["undo_business", "undo_log"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&mut observer)
                .await
                .unwrap(),
            0
        );
    }
    observer.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn inbound_foreign_key_on_undo_table_rejects_flush_and_rolls_back_business() {
    let mut admin = fixture::reset().await;
    sqlx::query("CREATE TABLE undo_child(xid VARCHAR(128),branch_id BIGINT,FOREIGN KEY(xid,branch_id) REFERENCES undo_log(xid,branch_id)) ENGINE=InnoDB").execute(&mut admin).await.unwrap();
    let (ds, mut tx) = fixture::prepared(None).await;
    let result = tx
        .flush_undo_logs("undo-store:xid", 42, MySqlJdbcProfile::Connector8Default)
        .await;
    assert!(
        matches!(result, Err(DbError::Unsupported { .. })),
        "incoming Undo reference must be rejected"
    );
    assert!(matches!(
        tx.table_meta("undo_work").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    fixture::dirty(&mut admin).await;
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn existing_normal_and_global_finished_rows_are_not_overwritten() {
    let mut admin = fixture::reset().await;
    for status in [0, 1] {
        sqlx::query(
            "INSERT INTO undo_log VALUES(42,'undo-store:xid','fixture',X'7B7D',?,now(6),now(6))",
        )
        .bind(status)
        .execute(&mut admin)
        .await
        .unwrap();
        let (ds, mut tx) = fixture::prepared(None).await;
        let error = tx
            .flush_undo_logs("undo-store:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            DbError::Driver {
                code: Some(1062),
                ..
            }
        ));
        assert!(!format!("{error:?}").contains("undo-store:xid"));
        assert!(matches!(
            tx.table_meta("undo_work").await,
            Err(DbError::Poisoned)
        ));
        tx.rollback().await.unwrap();
        ds.close().await.unwrap();
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
        let actual: i32 = sqlx::query_scalar("SELECT log_status FROM undo_log")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(actual, status);
        let bytes: Vec<u8> = sqlx::query_scalar("SELECT rollback_info FROM undo_log")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(bytes, b"{}");
        fixture::snapshot(&mut admin, &format!("existing-{status}.json"), status).await;
        sqlx::query("DELETE FROM undo_log")
            .execute(&mut admin)
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn empty_flush_writes_nothing_and_successful_flush_seals_all_mutations() {
    let mut admin = fixture::reset().await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    for work in [false, true] {
        let mut tx = ds.begin().await.unwrap();
        let sql = SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'once')")
            .unwrap();
        if work {
            tx.execute(&sql, &[]).await.unwrap();
        }
        tx.flush_undo_logs("undo-store:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap();
        assert!(matches!(
            tx.execute(&sql, &[]).await,
            Err(DbError::UndoSealed)
        ));
        assert!(matches!(
            tx.flush_undo_logs("another", 43, MySqlJdbcProfile::Connector8Default)
                .await,
            Err(DbError::UndoSealed)
        ));
        assert!(matches!(
            tx.table_meta("undo_work").await,
            Err(DbError::UndoSealed)
        ));
        let select = SqlRecognizerFactory::recognize("DELETE FROM undo_work WHERE id=1").unwrap();
        assert!(matches!(
            tx.before_image(&select, &[]).await,
            Err(DbError::UndoSealed)
        ));
        fixture::dirty(&mut admin).await;
        assert_eq!(
            fixture::count(&mut admin, "undo_log").await,
            i64::from(work)
        );
        tx.rollback().await.unwrap();
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
        assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    }
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn missing_unsafe_or_incomplete_undo_table_never_commits_business() {
    for bad in [
        "DROP TABLE undo_log",
        "ALTER TABLE undo_log ENGINE=MyISAM",
        "ALTER TABLE undo_log DROP INDEX ux_undo_log",
        "ALTER TABLE undo_log MODIFY context VARCHAR(127) NOT NULL",
        "CREATE TRIGGER undo_before BEFORE INSERT ON undo_log FOR EACH ROW SET NEW.log_status=1",
    ] {
        let mut admin = fixture::reset().await;
        sqlx::raw_sql(bad).execute(&mut admin).await.unwrap();
        let (ds, mut tx) = fixture::prepared(None).await;
        assert!(
            tx.flush_undo_logs("undo-store:xid", 42, MySqlJdbcProfile::Connector8Default)
                .await
                .is_err(),
            "unsafe Undo shape must fail"
        );
        assert!(matches!(
            tx.table_meta("undo_work").await,
            Err(DbError::Poisoned)
        ));
        tx.rollback().await.unwrap();
        ds.close().await.unwrap();
        fixture::dirty(&mut admin).await;
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
        if bad != "DROP TABLE undo_log" {
            assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
        }
    }
}

async fn blocked_flush(cancel: bool) {
    let mut admin = fixture::reset().await;
    let mut blocker = admin.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO undo_log VALUES(42,'undo-store:xid','fixture',X'7B7D',1,now(6),now(6))",
    )
    .execute(&mut *blocker)
    .await
    .unwrap();
    let (ds, mut tx) = fixture::prepared(if cancel {
        None
    } else {
        Some(std::time::Duration::from_millis(750))
    })
    .await;
    let mut observer = owned_mysql::admin().await;
    let mut operation =
        Box::pin(tx.flush_undo_logs("undo-store:xid", 42, MySqlJdbcProfile::Connector8Default));
    fixture::wait_blocked(
        &mut operation,
        &mut observer,
        if cancel {
            "wait-cancellation.json"
        } else {
            "wait-deadline.json"
        },
    )
    .await;
    if !cancel {
        assert!(matches!(
            operation.as_mut().await,
            Err(DbError::Deadline {
                operation: "Undo flush"
            })
        ));
    }
    drop(operation);
    assert!(matches!(
        tx.table_meta("undo_work").await,
        Err(DbError::Poisoned)
    ));
    assert!(matches!(tx.rollback().await, Err(DbError::Closed)));
    blocker.rollback().await.unwrap();
    ds.close().await.unwrap();
    fixture::dirty(&mut observer).await;
    for _ in 0..100 {
        if fixture::count(&mut observer, "undo_work").await == 0
            && fixture::count(&mut observer, "undo_log").await == 0
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("cancelled/deadline original transaction retained business/Undo effects");
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn cancelling_actual_unique_key_wait_discards_owned_transaction() {
    blocked_flush(true).await;
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn absolute_deadline_during_actual_unique_key_wait_discards_owned_transaction() {
    blocked_flush(false).await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn packet_splits_original_bytes_and_root_context_reassembles_exactly() {
    let mut admin = fixture::reset().await;
    sqlx::query("SET GLOBAL max_allowed_packet=16384")
        .execute(&mut admin)
        .await
        .unwrap();
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = format!(
        "INSERT INTO undo_work(id,v) VALUES{}",
        (1..=24)
            .map(|id| format!("({id},?)"))
            .collect::<Vec<_>>()
            .join(",")
    );
    tx.execute(
        &SqlRecognizerFactory::recognize(&sql).unwrap(),
        &vec![seata_datasource::FieldValue::Text("x".repeat(300)); 24],
    )
    .await
    .unwrap();
    let expected = seata_at::JacksonUndoLogParser::default()
        .encode_prepared(
            "undo-store:xid",
            42,
            tx.undo_logs(),
            MySqlJdbcProfile::Connector8Default,
        )
        .unwrap();
    tx.flush_undo_logs("undo-store:xid", 42, MySqlJdbcProfile::Connector8Default)
        .await
        .unwrap();
    fixture::dirty(&mut admin).await;
    let rows = fixture::snapshot(&mut admin, "chunks.json", 0).await;
    assert!(rows.len() > 1);
    let root = rows.iter().find(|r| r.0 == 42).unwrap();
    let context = fixture::context(&root.1);
    assert_eq!(context["map"], "16384");
    assert_eq!(context["compressorType"], "NONE");
    assert_eq!(context["serializer"], "jackson");
    let ids: Vec<i64> = context["subId"]
        .split(',')
        .filter(|v| !v.is_empty())
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(rows.len(), ids.len() + 1);
    let mut joined = root.2.clone();
    for id in ids {
        let part = rows.iter().find(|r| r.0 == id).unwrap();
        assert_eq!(part.1, "branchId=42");
        assert!(part.2.len() <= 13107);
        joined.extend_from_slice(&part.2);
    }
    assert_eq!(
        joined, expected,
        "all original bytes must survive packet segmentation"
    );
    fixture::save("chunk-payload.json", &serde_json::json!({"bytes":expected}));
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    sqlx::query("SET GLOBAL max_allowed_packet=67108864")
        .execute(&mut admin)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn oversized_sub_id_context_never_publishes_partial_undo() {
    let mut admin = fixture::reset().await;
    sqlx::query("SET GLOBAL max_allowed_packet=16384")
        .execute(&mut admin)
        .await
        .unwrap();
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    for batch in 0..8 {
        let sql = format!(
            "INSERT INTO undo_work(id,v) VALUES{}",
            (1..=16)
                .map(|id| format!("({},?)", batch * 16 + id))
                .collect::<Vec<_>>()
                .join(",")
        );
        tx.execute(
            &SqlRecognizerFactory::recognize(&sql).unwrap(),
            &vec![seata_datasource::FieldValue::Text("x".repeat(300)); 16],
        )
        .await
        .unwrap();
    }
    assert!(matches!(
        tx.flush_undo_logs("undo-store:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await,
        Err(DbError::Unsupported {
            feature: "Undo subId context capacity"
        })
    ));
    fixture::dirty(&mut admin).await;
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 128);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    assert!(matches!(
        tx.table_meta("undo_work").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    sqlx::query("SET GLOBAL max_allowed_packet=67108864")
        .execute(&mut admin)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn invalid_original_identity_poisoned_before_any_undo_insert() {
    for (xid, branch) in [("", 42), ("undo-store:xid", -1)] {
        let mut admin = fixture::reset().await;
        let (ds, mut tx) = fixture::prepared(None).await;
        assert!(matches!(
            tx.flush_undo_logs(xid, branch, MySqlJdbcProfile::Connector8Default)
                .await,
            Err(DbError::UndoCodec(_))
        ));
        assert!(matches!(
            tx.table_meta("undo_work").await,
            Err(DbError::Poisoned)
        ));
        fixture::dirty(&mut admin).await;
        assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
        tx.rollback().await.unwrap();
        ds.close().await.unwrap();
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    }
}
