//! 原物理日志的真实数据库补偿，不提供原业务提交入口。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/undo_rollback_fixture.rs"]
mod rollback_fixture;
#[path = "support/undo_rollback_wait.rs"]
mod rollback_wait;
use seata_at::{
    BaseTransactionalExecutor, JacksonUndoLogParser, MySqlJdbcProfile, SqlRecognizerFactory,
};
use seata_datasource::FieldValue;
use seata_db_mysql::{DbError, MySqlDataSource, UndoRollbackError, UndoRollbackObservation};
use sqlx::Connection;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn trigger_committed_after_undo_dictionary_snapshot_cannot_execute_during_compensation() {
    assert_late_undo_constraint_rejected(
        "trigger",
        "CREATE TRIGGER late_undo_trigger AFTER DELETE ON undo_work FOR EACH ROW INSERT INTO undo_effect VALUES(OLD.id)",
        "table trigger",
    ).await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn foreign_keys_committed_after_undo_dictionary_snapshot_cannot_affect_compensation() {
    for (case, change) in [
        ("outbound", "ALTER TABLE undo_work ADD CONSTRAINT late_undo_outbound FOREIGN KEY(id) REFERENCES undo_reference(id)"),
        ("inbound", "ALTER TABLE undo_child ADD CONSTRAINT late_undo_inbound FOREIGN KEY(v) REFERENCES undo_work(id) ON DELETE CASCADE"),
    ] {
        assert_late_undo_constraint_rejected(case, change, "inbound/outbound foreign key").await;
    }
}

async fn assert_late_undo_constraint_rejected(case: &str, change: &str, expected: &str) {
    use std::time::Duration;

    let (mut admin, ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS undo_effect,undo_reference;
        CREATE TABLE undo_effect(id INT) ENGINE=InnoDB;
        CREATE TABLE undo_reference(id INT PRIMARY KEY) ENGINE=InnoDB;
        CREATE TABLE undo_child(id INT PRIMARY KEY,v INT) ENGINE=InnoDB;
        INSERT INTO undo_reference VALUES(1);
        INSERT INTO undo_child VALUES(2,1)",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    let before = rollback_fixture::snapshot(&mut admin).await;
    let mut locker = owned_mysql::admin().await;
    sqlx::raw_sql(
        "LOCK TABLES undo_work WRITE,undo_effect WRITE,undo_reference WRITE,undo_child WRITE",
    )
    .execute(&mut locker)
    .await
    .unwrap();
    let mut rollback =
        Box::pin(ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    // 先从服务器证明补偿已读原 Undo 字典且正在等待业务表 MDL，再发布约束。
    let waits = loop {
        tokio::select! {
            result = rollback.as_mut() => panic!("rollback ended before business MDL wait: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(5)) => {}
        }
        let waits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM performance_schema.metadata_locks
             WHERE OBJECT_SCHEMA=DATABASE() AND OBJECT_NAME='undo_work' AND LOCK_STATUS='PENDING'",
        )
        .fetch_one(&mut admin)
        .await
        .unwrap();
        if waits > 0 {
            break waits;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "business MDL wait missing"
        );
    };
    sqlx::raw_sql(change).execute(&mut locker).await.unwrap();
    sqlx::raw_sql("UNLOCK TABLES")
        .execute(&mut locker)
        .await
        .unwrap();
    let result = rollback.await;
    fixture::save(
        &format!("late-undo-{case}.json"),
        &serde_json::json!({
            "actual_metadata_waits":waits,"result":format!("{result:?}"),
        }),
    );
    assert_eq!(
        result.as_ref().unwrap_err().identity(),
        Some(("rollback:xid", 42))
    );
    assert!(
        matches!(result, Err(UndoRollbackError::BeforeCommit {
        source: DbError::Unsupported { feature }, ..
    }) if feature == expected),
        "late {case} was allowed to participate in compensation: {result:?}"
    );
    assert_eq!(before, rollback_fixture::snapshot(&mut admin).await);
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut admin, "undo_effect").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_child").await, 1);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn stored_insert_undo_restores_business_and_consumes_original_row_atomically() {
    let mut admin = fixture::reset().await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let original = "INSERT INTO undo_work(id,v) VALUES(1,'created')";
    tx.execute(&SqlRecognizerFactory::recognize(original).unwrap(), &[])
        .await
        .unwrap();
    let payload = JacksonUndoLogParser::default()
        .encode_prepared(
            "rollback:xid",
            42,
            tx.undo_logs(),
            MySqlJdbcProfile::Connector8Default,
        )
        .unwrap();
    tx.rollback().await.unwrap();
    // 原生准备字节/物理后镜像重建为独立已提交夹具；正式API仍无原业务COMMIT。
    let mut seed = admin.begin().await.unwrap();
    sqlx::query(original).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO undo_log VALUES(42,'rollback:xid','serializer=jackson&compressorType=NONE&map=67108864',?,0,now(6),now(6))").bind(&payload).execute(&mut *seed).await.unwrap();
    seed.commit().await.unwrap();
    fixture::save("source-payload.json", &serde_json::json!({"bytes":payload}));
    assert_eq!(
        ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 1,
            restored_rows: 1
        }
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    fixture::save(
        "restored-insert.json",
        &serde_json::json!({"business_rows":0,"undo_rows":0,"xid":"rollback:xid","branch_id":42}),
    );
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn corrupted_or_missing_subrows_keep_business_and_every_remaining_original_byte() {
    for mutation in [
        "DELETE FROM undo_log WHERE branch_id=?",
        "UPDATE undo_log SET xid='other:xid' WHERE branch_id=?",
        "UPDATE undo_log SET log_status=1 WHERE branch_id=?",
        "UPDATE undo_log SET context='branchId=43' WHERE branch_id=?",
        "duplicate",
        "orphan",
    ] {
        let statements: Vec<String> = (1..=16)
            .map(|id| {
                format!(
                    "INSERT INTO undo_work(id,v) VALUES({id},'{}')",
                    "v".repeat(700)
                )
            })
            .collect();
        let refs: Vec<_> = statements.iter().map(String::as_str).collect();
        let (mut admin, ds) =
            rollback_fixture::seed(&[], &refs, MySqlJdbcProfile::Connector8Default, Some(16384))
                .await;
        let rows = rollback_fixture::snapshot(&mut admin).await;
        assert!(rows.len() > 1);
        let child = rows
            .iter()
            .find(|r| r["branch_id"].as_i64() != Some(42))
            .unwrap()["branch_id"]
            .as_i64()
            .unwrap();
        if mutation == "duplicate" {
            sqlx::query("UPDATE undo_log SET context=? WHERE branch_id=42")
                .bind(format!(
                    "serializer=jackson&compressorType=NONE&map=16384&subId={child},{child},"
                ))
                .execute(&mut admin)
                .await
                .unwrap();
        } else if mutation == "orphan" {
            sqlx::query("INSERT INTO undo_log VALUES(999,'rollback:xid','branchId=42',X'7B7D',0,now(6),now(6))").execute(&mut admin).await.unwrap();
        } else {
            sqlx::query(mutation)
                .bind(child)
                .execute(&mut admin)
                .await
                .unwrap();
        }
        let before = rollback_fixture::snapshot(&mut admin).await;
        assert!(
            matches!(
                ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
                    .await,
                Err(UndoRollbackError::BeforeCommit { .. })
            ),
            "bad subrow accepted: {mutation}"
        );
        assert_eq!(before, rollback_fixture::snapshot(&mut admin).await);
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 16);
        ds.close().await.unwrap();
    }
    fixture::save(
        "subrow-negative-results.json",
        &serde_json::json!({"cases":6,"all_rejected_before_commit":true,"business_and_remaining_undo_unchanged":true}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn orphan_undo_children_prevent_empty_rollback_with_or_without_guard() {
    let mut admin = fixture::reset().await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    sqlx::raw_sql(
        "INSERT INTO undo_work VALUES(1,'original-business');
         INSERT INTO undo_log VALUES
           (101,'rollback:missing-root','branchId=42',X'0102',0,now(6),now(6)),
           (102,'rollback:existing-guard','branchId=42',X'0304',0,now(6),now(6)),
           (42,'rollback:existing-guard','serializer=jackson&compressorType=NONE',X'7B7D',1,now(6),now(6)),
           (103,'rollback:empty','branchId=43',X'0506',0,now(6),now(6));",
    )
    .execute(&mut admin)
    .await
    .unwrap();

    let original = rollback_fixture::snapshot(&mut admin).await;
    let rejections = [
        ds.rollback_undo_log(
            "rollback:missing-root",
            42,
            MySqlJdbcProfile::Connector8Default,
        )
        .await,
        ds.rollback_undo_log(
            "rollback:existing-guard",
            42,
            MySqlJdbcProfile::Connector8Default,
        )
        .await,
    ];
    assert!(
        rejections.iter().all(|error| matches!(
            error,
            Err(UndoRollbackError::BeforeCommit {
                source: DbError::Unsupported {
                    feature: "Undo orphan children without Normal root"
                },
                ..
            })
        )),
        "rollback must leave both orphan branches unresolved: {rejections:?}"
    );
    assert_eq!(original, rollback_fixture::snapshot(&mut admin).await);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT v FROM undo_work WHERE id=1")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        "original-business"
    );
    for inserted in [true, false] {
        assert_eq!(
            ds.rollback_undo_log("rollback:empty", 42, MySqlJdbcProfile::Connector8Default)
                .await
                .unwrap(),
            UndoRollbackObservation::Fenced { inserted }
        );
    }
    let mut rows = rollback_fixture::snapshot(&mut admin).await;
    rows.retain(|row| row["xid"] != "rollback:empty" || row["branch_id"] != 42);
    assert_eq!(rows, original);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn changed_or_unsafe_schema_and_unknown_compression_do_not_consume_undo() {
    for change in ["ALTER TABLE undo_work ADD extra INT","ALTER TABLE undo_work ENGINE=MyISAM","CREATE TRIGGER undo_business_delete BEFORE DELETE ON undo_work FOR EACH ROW SET @rollback_side_effect=1","UPDATE undo_log SET context='serializer=jackson&compressorType=GZIP'"] {
        let (mut admin,ds)=rollback_fixture::seed(&[],&["INSERT INTO undo_work(id,v) VALUES(1,'created')"],MySqlJdbcProfile::Connector8Default,None).await;
        sqlx::raw_sql(change).execute(&mut admin).await.unwrap();let before=rollback_fixture::snapshot(&mut admin).await;
        assert!(matches!(ds.rollback_undo_log("rollback:xid",42,MySqlJdbcProfile::Connector8Default).await,Err(UndoRollbackError::BeforeCommit{..})),"unsafe shape accepted: {change}");
        assert_eq!(before,rollback_fixture::snapshot(&mut admin).await);assert_eq!(fixture::count(&mut admin,"undo_work").await,1);ds.close().await.unwrap();
    }
    fixture::save(
        "schema-negative-results.json",
        &serde_json::json!({"cases":4,"original_undo_unchanged":true,"business_unchanged":true}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn cancellation_during_original_undo_lock_wait_publishes_no_partial_restoration() {
    rollback_wait::blocked_rollback(true).await;
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn deadline_during_original_undo_lock_wait_publishes_no_partial_restoration() {
    rollback_wait::blocked_rollback(false).await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn concurrent_duplicate_rollback_has_one_normal_consumer_and_preserves_original_missing_log_semantics(
) {
    let (mut admin, ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    let (a, b) = tokio::join!(
        ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default),
        ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
    );
    let results = [a, b];
    fixture::save(
        "concurrent-rollback-results.json",
        &serde_json::json!({"results":results.iter().map(|r|format!("{r:?}")).collect::<Vec<_>>()}),
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(
                r,
                Ok(UndoRollbackObservation::Consumed {
                    logs: 1,
                    restored_rows: 1
                })
            ))
            .count(),
        1
    );
    for r in results {
        assert!(
            matches!(
                r,
                Ok(UndoRollbackObservation::Consumed { .. }
                    | UndoRollbackObservation::Fenced { .. })
                    | Err(UndoRollbackError::BeforeCommit {
                        source: DbError::Driver {
                            code: Some(1213 | 1062),
                            ..
                        },
                        ..
                    })
            ),
            "unexpected concurrent rollback: {r:?}"
        );
    }
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(
        ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Fenced { inserted: false }
    );
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
#[allow(clippy::type_complexity)]
async fn update_and_delete_restore_original_values_without_pk_cartesian_products() {
    for statement in ["UPDATE undo_business SET v='changed',amount=amount+1,payload=NULL WHERE (a='x_y' AND z=1) OR (a='x' AND z=2)","DELETE FROM undo_business WHERE (a='x_y' AND z=1) OR (a='x' AND z=2)"] {
        let (mut admin,ds)=rollback_fixture::seed(&["CREATE TABLE undo_business(z INT NOT NULL,a VARCHAR(20) NOT NULL,v VARCHAR(30),amount DECIMAL(65,30),payload VARBINARY(20),PRIMARY KEY(a,z)) ENGINE=InnoDB","INSERT INTO undo_business VALUES(1,'x_y','original',123.000000000000000000000000000001,X'00FF'),(2,'x',NULL,2.100000000000000000000000000000,X'0000'),(2,'x_y','keep',3.000000000000000000000000000000,NULL)"],&[statement],MySqlJdbcProfile::Connector8Default,None).await;
        assert_eq!(ds.rollback_undo_log("rollback:xid",42,MySqlJdbcProfile::Connector8Default).await.unwrap(),UndoRollbackObservation::Consumed{logs:1,restored_rows:2});
        let rows:Vec<(i32,String,Option<String>,String,Option<String>)>=sqlx::query_as("SELECT z,a,v,CAST(amount AS CHAR),HEX(payload) FROM undo_business ORDER BY a,z").fetch_all(&mut admin).await.unwrap();
        assert_eq!(rows,vec![(2,"x".into(),None,"2.100000000000000000000000000000".into(),Some("0000".into())),(1,"x_y".into(),Some("original".into()),"123.000000000000000000000000000001".into(),Some("00FF".into())),(2,"x_y".into(),Some("keep".into()),"3.000000000000000000000000000000".into(),None)]);
        assert_eq!(fixture::count(&mut admin,"undo_log").await,0);ds.close().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn mixed_logs_run_in_reverse_and_consume_only_original_branch() {
    let (mut admin, ds) = rollback_fixture::seed(
        &["INSERT INTO undo_work VALUES(1,'before')"],
        &[
            "UPDATE undo_work SET v='after' WHERE id=1",
            "INSERT INTO undo_work(id,v) VALUES(2,'new')",
            "DELETE FROM undo_work WHERE id=1",
        ],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    sqlx::query("INSERT INTO undo_log VALUES(999,'other:xid','foreign',X'7B7D',0,now(6),now(6))")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 3,
            restored_rows: 3
        }
    );
    let rows: Vec<(i32, String)> = sqlx::query_as("SELECT id,v FROM undo_work ORDER BY id")
        .fetch_all(&mut admin)
        .await
        .unwrap();
    assert_eq!(rows, vec![(1, "before".into())]);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT xid FROM undo_log")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        "other:xid"
    );
    fixture::save(
        "mixed-reversed.json",
        &serde_json::json!({"business":rows,"remaining_undo":rollback_fixture::snapshot(&mut admin).await}),
    );
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn later_dirty_failure_rolls_back_earlier_compensation_and_keeps_all_undo_bytes() {
    let (mut admin, ds) = rollback_fixture::seed(
        &["INSERT INTO undo_work VALUES(1,'before')"],
        &[
            "UPDATE undo_work SET v='after' WHERE id=1",
            "INSERT INTO undo_work(id,v) VALUES(2,'new')",
        ],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    sqlx::query("UPDATE undo_work SET v='external' WHERE id=1")
        .execute(&mut admin)
        .await
        .unwrap();
    let before = rollback_fixture::snapshot(&mut admin).await;
    assert!(matches!(
        ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await,
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::UndoDirty,
            ..
        })
    ));
    assert_eq!(before, rollback_fixture::snapshot(&mut admin).await);
    let rows: Vec<(i32, String)> = sqlx::query_as("SELECT id,v FROM undo_work ORDER BY id")
        .fetch_all(&mut admin)
        .await
        .unwrap();
    assert_eq!(rows, vec![(1, "external".into()), (2, "new".into())]);
    fixture::save(
        "dirty-rollback.json",
        &serde_json::json!({"business":rows,"original_undo_unchanged":true}),
    );
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn already_restored_and_equal_images_skip_dml_then_follow_original_missing_log_fence() {
    for unchanged in [false, true] {
        let statement = if unchanged {
            "UPDATE undo_work SET v='before' WHERE id=1"
        } else {
            "UPDATE undo_work SET v='after' WHERE id=1"
        };
        let (mut admin, ds) = rollback_fixture::seed(
            &["INSERT INTO undo_work VALUES(1,'before')"],
            &[statement],
            MySqlJdbcProfile::Connector8Default,
            None,
        )
        .await;
        sqlx::query("UPDATE undo_work SET v='before' WHERE id=1")
            .execute(&mut admin)
            .await
            .unwrap();
        assert_eq!(
            ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
                .await
                .unwrap(),
            UndoRollbackObservation::Consumed {
                logs: 1,
                restored_rows: 0
            }
        );
        assert_eq!(
            ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
                .await
                .unwrap(),
            UndoRollbackObservation::Fenced { inserted: true }
        );
        assert_eq!(
            ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
                .await
                .unwrap(),
            UndoRollbackObservation::Fenced { inserted: false }
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT v FROM undo_work")
                .fetch_one(&mut admin)
                .await
                .unwrap(),
            "before"
        );
        ds.close().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn real_packet_chunks_are_validated_reassembled_and_deleted_with_their_original_root() {
    let statements: Vec<String> = (1..=16)
        .map(|id| {
            format!(
                "INSERT INTO undo_work(id,v) VALUES({id},'{}')",
                "v".repeat(700)
            )
        })
        .collect();
    let refs: Vec<_> = statements.iter().map(String::as_str).collect();
    let (mut admin, ds) =
        rollback_fixture::seed(&[], &refs, MySqlJdbcProfile::Connector8Default, Some(16384)).await;
    let before = rollback_fixture::snapshot(&mut admin).await;
    assert!(before.len() > 1, "actual SQLx flush must create subrows");
    assert_eq!(
        ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 16,
            restored_rows: 16
        }
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    fixture::save(
        "chunks-consumed.json",
        &serde_json::json!({"original_rows":before,"after_business":0,"after_undo":0}),
    );
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn jdbc_profile_or_original_document_identity_mismatch_never_changes_business() {
    let (mut admin, ds) = rollback_fixture::seed(
        &["CREATE TABLE undo_business(id INT PRIMARY KEY,b TINYINT(1)) ENGINE=InnoDB"],
        &["INSERT INTO undo_business(id,b) VALUES(1,1)"],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    let before = rollback_fixture::snapshot(&mut admin).await;
    assert!(matches!(
        ds.rollback_undo_log(
            "rollback:xid",
            42,
            MySqlJdbcProfile::Connector8NumericTinyInt
        )
        .await,
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::UndoCodec(_),
            ..
        })
    ));
    assert_eq!(before, rollback_fixture::snapshot(&mut admin).await);
    assert_eq!(fixture::count(&mut admin, "undo_business").await, 1);
    sqlx::query("UPDATE undo_log SET xid='other:identity'")
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(matches!(
        ds.rollback_undo_log("other:identity", 42, MySqlJdbcProfile::Connector8Default)
            .await,
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::Input {
                reason: "Undo original document identity"
            },
            ..
        })
    ));
    assert_eq!(fixture::count(&mut admin, "undo_business").await, 1);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn numeric_tinyint_profile_restores_physical_integer_two_without_boolean_loss() {
    let (mut admin, ds) = rollback_fixture::seed(
        &[
            "CREATE TABLE undo_business(id INT PRIMARY KEY,b TINYINT(1)) ENGINE=InnoDB",
            "INSERT INTO undo_business VALUES(1,2)",
        ],
        &["UPDATE undo_business SET b=1 WHERE id=1"],
        MySqlJdbcProfile::Connector8NumericTinyInt,
        None,
    )
    .await;
    assert_eq!(
        ds.rollback_undo_log(
            "rollback:xid",
            42,
            MySqlJdbcProfile::Connector8NumericTinyInt
        )
        .await
        .unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 1,
            restored_rows: 1
        }
    );
    assert_eq!(
        sqlx::query_scalar::<_, i8>("SELECT b FROM undo_business")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        2
    );
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn binary_collation_text_keeps_utf8_keys_and_restores_the_original_row() {
    let mut admin = fixture::reset().await;
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS bin_text_image;
         CREATE TABLE bin_text_image(
            id VARCHAR(36) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin PRIMARY KEY,
            note TEXT CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_bin,
            tag VARCHAR(12) CHARACTER SET ascii COLLATE ascii_bin,
            legacy VARCHAR(12) CHARACTER SET utf8mb3 COLLATE utf8mb3_bin,
            payload VARBINARY(8)
         ) ENGINE=InnoDB",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    let upper = "A𐐀";
    let lower = "a𐐀";
    let original_note = "Cafe\u{301}";
    let changed_note = "café🦀";
    let legacy_text = "原字符";
    let initial = "INSERT INTO bin_text_image(id,note,tag,legacy,payload) VALUES(?,?,?,?,?)";
    sqlx::query(initial)
        .bind(upper)
        .bind(original_note)
        .bind("Alpha ")
        .bind(legacy_text)
        .bind(&[0, 255][..])
        .execute(&mut admin)
        .await
        .unwrap();

    let profile = MySqlJdbcProfile::Connector8Default;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let update = "UPDATE bin_text_image SET note=?,tag=NULL,legacy=NULL WHERE id=?";
    tx.execute(
        &SqlRecognizerFactory::recognize(update).unwrap(),
        &[
            FieldValue::Text(changed_note.into()),
            FieldValue::Text(upper.into()),
        ],
    )
    .await
    .expect("binary-collation UPDATE must decode real text before and after");
    tx.execute(
        &SqlRecognizerFactory::recognize(initial).unwrap(),
        &[
            FieldValue::Text(lower.into()),
            FieldValue::Null,
            FieldValue::Text(String::new()),
            FieldValue::Text("新增".into()),
            FieldValue::Binary(vec![0, 255, 0]),
        ],
    )
    .await
    .expect("binary-collation INSERT must decode the temporary key and final image");

    let logs = tx.undo_logs();
    assert_eq!(logs.len(), 2);
    let before = &logs[0].before_image().rows()[0];
    let after = &logs[0].after_image().rows()[0];
    let inserted = &logs[1].after_image().rows()[0];
    assert_eq!(
        before.field("id").unwrap().value(),
        &FieldValue::Text(upper.into())
    );
    assert_eq!(
        before.field("note").unwrap().value(),
        &FieldValue::Text(original_note.into())
    );
    assert_eq!(
        before.field("tag").unwrap().value(),
        &FieldValue::Text("Alpha ".into())
    );
    assert_eq!(
        before.field("payload").unwrap().value(),
        &FieldValue::Binary(vec![0, 255])
    );
    assert_eq!(
        after.field("note").unwrap().value(),
        &FieldValue::Text(changed_note.into())
    );
    assert_eq!(after.field("tag").unwrap().value(), &FieldValue::Null);
    assert_eq!(
        before.field("legacy").unwrap().value(),
        &FieldValue::Text(legacy_text.into())
    );
    assert_eq!(after.field("legacy").unwrap().value(), &FieldValue::Null);
    assert_eq!(
        inserted.field("legacy").unwrap().value(),
        &FieldValue::Text("新增".into())
    );
    assert_eq!(
        inserted.field("id").unwrap().value(),
        &FieldValue::Text(lower.into())
    );
    assert_eq!(inserted.field("note").unwrap().value(), &FieldValue::Null);
    assert_eq!(
        inserted.field("tag").unwrap().value(),
        &FieldValue::Text(String::new())
    );
    assert_eq!(
        inserted.field("payload").unwrap().value(),
        &FieldValue::Binary(vec![0, 255, 0])
    );
    let keys: Vec<_> = logs
        .iter()
        .map(|log| {
            BaseTransactionalExecutor::build_lock_key(log.after_image(), profile)
                .unwrap()
                .unwrap()
        })
        .collect();
    assert_eq!(
        keys,
        [
            format!("bin_text_image:{upper}"),
            format!("bin_text_image:{lower}")
        ]
    );

    let xid = "rollback:bin-text";
    let payload = JacksonUndoLogParser::default()
        .encode_prepared(xid, 42, logs, profile)
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "bin_text_image").await, 1);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);

    // 只由自有管理事务重建已提交场景，运行正式补偿入口。
    let mut seed = admin.begin().await.unwrap();
    sqlx::query(update)
        .bind(changed_note)
        .bind(upper)
        .execute(&mut *seed)
        .await
        .unwrap();
    sqlx::query(initial)
        .bind(lower)
        .bind(None::<&str>)
        .bind("")
        .bind("新增")
        .bind(&[0, 255, 0][..])
        .execute(&mut *seed)
        .await
        .unwrap();
    sqlx::query("INSERT INTO undo_log VALUES(42,?,'serializer=jackson&compressorType=NONE&map=67108864',?,0,now(6),now(6))")
        .bind(xid).bind(&payload).execute(&mut *seed).await.unwrap();
    seed.commit().await.unwrap();

    assert_eq!(
        ds.rollback_undo_log(xid, 42, profile).await.unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 2,
            restored_rows: 2
        }
    );
    type RestoredBytes = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);
    let restored: RestoredBytes = sqlx::query_as(
        "SELECT CAST(id AS BINARY),CAST(note AS BINARY),CAST(tag AS BINARY),CAST(legacy AS BINARY),payload FROM bin_text_image",
    ).fetch_one(&mut admin).await.unwrap();
    assert_eq!(
        restored,
        (
            upper.as_bytes().to_vec(),
            original_note.as_bytes().to_vec(),
            b"Alpha ".to_vec(),
            legacy_text.as_bytes().to_vec(),
            vec![0, 255],
        )
    );
    assert_eq!(fixture::count(&mut admin, "bin_text_image").await, 1);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    fixture::save(
        "restored-bin-text.json",
        &serde_json::json!({
            "keys":keys,"restored_id":restored.0,"restored_note":restored.1,"restored_tag":restored.2,
            "restored_legacy":restored.3,"restored_binary":restored.4,"undo_rows":0,"restored_rows":2
        }),
    );
    ds.close().await.unwrap();
}
