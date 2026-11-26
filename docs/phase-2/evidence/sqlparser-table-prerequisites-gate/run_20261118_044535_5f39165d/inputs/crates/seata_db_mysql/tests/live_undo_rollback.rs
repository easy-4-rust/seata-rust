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
use seata_at::{JacksonUndoLogParser, MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{DbError, MySqlDataSource, UndoRollbackError, UndoRollbackObservation};
use sqlx::Connection;

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
