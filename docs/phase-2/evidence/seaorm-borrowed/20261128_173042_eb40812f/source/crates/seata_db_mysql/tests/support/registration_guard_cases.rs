//! 自有 MySQL 生成主键镜像。受控 TCP 对端只收到可完整传输的正式 BranchRegister。
use super::{
    first_fixture::{self, Harness},
    fixture, owned_mysql, recovery, rm_fixture,
};
use seata_at::{LockKeyError, MySqlJdbcProfile, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use seata_db_mysql::{DbError, MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::Message;
use seata_rm::BranchResource;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn lossy_row_tokens_abort_before_registration_and_safe_tokens_keep_the_original_wire() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_millis(700)).await;
    sqlx::query("DROP TABLE IF EXISTS lock_text,lock_binary")
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE lock_text(id VARCHAR(32) PRIMARY KEY) ENGINE=InnoDB COLLATE=utf8mb4_0900_as_cs")
        .execute(&mut h.admin).await.unwrap();
    sqlx::query("CREATE TABLE lock_binary(id VARBINARY(8) PRIMARY KEY) ENGINE=InnoDB")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let mut socket = h.accept().await;
    let client = h.rm.client();
    let negatives = [
        (
            "empty",
            "INSERT INTO lock_text(id) VALUES(?)",
            vec![FieldValue::Text(String::new())],
        ),
        (
            "jdk-whitespace",
            "INSERT INTO lock_text(id) VALUES(?)",
            vec![FieldValue::Text("\u{2003}".into())],
        ),
        (
            "mixed-blank",
            "INSERT INTO lock_text(id) VALUES(?),(?)",
            vec![
                FieldValue::Text("".into()),
                FieldValue::Text("valid".into()),
            ],
        ),
        (
            "semicolon",
            "INSERT INTO lock_text(id) VALUES(?)",
            vec![FieldValue::Text("a;b".into())],
        ),
        (
            "binary-list",
            "INSERT INTO lock_binary(id) VALUES(?)",
            vec![FieldValue::Binary(vec![0, 255])],
        ),
    ];
    for (name, statement, values) in negatives {
        let mut tx = h.source.begin().await.unwrap();
        let affected = tx
            .execute(
                &SqlRecognizerFactory::recognize(statement).unwrap(),
                &values,
            )
            .await
            .unwrap();
        assert_eq!(affected, values.len() as u64);
        let failure = h
            .adapter
            .prepare_transaction(tx, &client, &format!("guard:{name}"), None)
            .await
            .unwrap_err();
        assert!(
            matches!(failure, MySqlPrepareError::LockKey(_)),
            "{name}: {failure:?}"
        );
        assert!(!failure.is_registration_unknown());
        assert!(failure.registered_branch().is_none());
    }
    let mut tx = h.source.begin().await.unwrap();
    for (statement, value) in [
        (
            "INSERT INTO lock_text(id) VALUES(?)",
            FieldValue::Text("\u{00a0}".into()),
        ),
        (
            "INSERT INTO lock_binary(id) VALUES(?)",
            FieldValue::Binary(vec![255]),
        ),
    ] {
        tx.execute(
            &SqlRecognizerFactory::recognize(statement).unwrap(),
            &[value],
        )
        .await
        .unwrap();
    }
    let reply = async {
        let frame = rm_fixture::peer::read(&mut socket).await;
        let Some(Message::BranchRegisterRequest(body)) = frame.body else {
            panic!("expected the first branch request");
        };
        assert_eq!(body.xid.as_deref(), Some("guard:safe"));
        assert_eq!(
            body.lock_key.as_deref(),
            Some("lock_binary:[-1];lock_text:\u{00a0}")
        );
        first_fixture::positive(&mut socket, frame.id).await;
    };
    let (prepared, ()) = tokio::join!(
        h.adapter
            .prepare_transaction(tx, &client, "guard:safe", None),
        reply
    );
    let prepared = prepared.unwrap();
    assert_eq!(
        prepared.registered_branch().unwrap().identity().branch_id(),
        42
    );
    prepared.rollback().await.unwrap();
    h.rm.shutdown().await.unwrap();
    rm_fixture::peer::eof(&mut socket).await;
    h.source.close().await.unwrap();
    for table in ["lock_text", "lock_binary", "undo_log"] {
        assert_eq!(fixture::count(&mut h.admin, table).await, 0);
    }
    assert_eq!(recovery::state(&mut h.admin).await, (1, 0, 0));
    fixture::save(
        "registration-lock-guard.json",
        &serde_json::json!({
            "rejected_cases":5,"first_wire_xid":"guard:safe","branch_requests":1,
            "positive_lock_key":"lock_binary:[-1];lock_text:\u{00a0}",
            "local_business_and_undo_rolled_back":true
        }),
    );
    composite_identity_is_rejected_locally_without_sending_or_persisting_a_branch().await;
    required_resource_binding_rejects_missing_and_different_ids_before_wire().await;
}

async fn composite_identity_is_rejected_locally_without_sending_or_persisting_a_branch() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(1)).await;
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS lock_composite,lock_mixed;
         CREATE TABLE lock_composite(a VARCHAR(32), b VARCHAR(32), PRIMARY KEY(a,b))
             ENGINE=InnoDB COLLATE=utf8mb4_0900_as_cs;
         CREATE TABLE lock_mixed(a VARCHAR(32), b BIGINT, PRIMARY KEY(a,b))
             ENGINE=InnoDB COLLATE=utf8mb4_0900_as_cs;",
    )
    .execute(&mut h.admin)
    .await
    .unwrap();
    let mut socket = h.accept().await;
    let client = h.rm.client();

    for (xid, statement, count) in [
        (
            "guard:left",
            "INSERT INTO lock_composite(a,b) VALUES('a_b','c')",
            1,
        ),
        (
            "guard:right",
            "INSERT INTO lock_composite(a,b) VALUES('a','b_c')",
            1,
        ),
        (
            "guard:multiple",
            "INSERT INTO lock_composite(a,b) VALUES('a','b'),('a','b_c')",
            2,
        ),
    ] {
        let mut tx = h.source.begin().await.unwrap();
        let cursor = SqlRecognizerFactory::recognize(statement).unwrap();
        assert_eq!(tx.execute(&cursor, &[]).await.unwrap(), count);
        let failure = h
            .adapter
            .prepare_transaction(tx, &client, xid, None)
            .await
            .unwrap_err();
        assert!(matches!(
            failure,
            MySqlPrepareError::LockKey(LockKeyError::Unsupported {
                feature: "ambiguous delimiter in composite primary key",
            })
        ));
        assert!(!failure.is_registration_unknown());
        assert!(failure.registered_branch().is_none());
    }

    let mut tx = h.source.begin().await.unwrap();
    for statement in [
        "INSERT INTO lock_composite(a,b) VALUES('a','b')",
        "INSERT INTO lock_mixed(a,b) VALUES('a_b',1)",
    ] {
        let cursor = SqlRecognizerFactory::recognize(statement).unwrap();
        assert_eq!(tx.execute(&cursor, &[]).await.unwrap(), 1);
    }
    let expected_key = "lock_composite:a_b;lock_mixed:a_b_1";
    let reply = async {
        let frame = rm_fixture::peer::read(&mut socket).await;
        let Some(Message::BranchRegisterRequest(body)) = frame.body else {
            panic!("expected the first branch request");
        };
        assert_eq!(body.xid.as_deref(), Some("guard:composite-safe"));
        assert_eq!(body.lock_key.as_deref(), Some(expected_key));
        first_fixture::positive(&mut socket, frame.id).await;
    };
    let (prepared, ()) = tokio::join!(
        h.adapter
            .prepare_transaction(tx, &client, "guard:composite-safe", None),
        reply
    );
    let prepared = prepared.unwrap();
    assert_eq!(
        prepared.registered_branch().unwrap().identity().branch_id(),
        42
    );
    prepared.rollback().await.unwrap();
    h.rm.shutdown().await.unwrap();
    rm_fixture::peer::eof(&mut socket).await;
    h.source.close().await.unwrap();
    for table in ["lock_composite", "lock_mixed", "undo_log"] {
        assert_eq!(fixture::count(&mut h.admin, table).await, 0);
    }
    assert_eq!(recovery::state(&mut h.admin).await, (1, 0, 0));
    fixture::save(
        "composite-registration-guard.json",
        &serde_json::json!({
            "rejected_cases": 3,
            "first_wire_xid": "guard:composite-safe",
            "branch_requests": 1,
            "positive_lock_key": expected_key,
            "business_and_undo_rows_after_rollback": 0
        }),
    );
}

async fn required_resource_binding_rejects_missing_and_different_ids_before_wire() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(1)).await;
    sqlx::query("DROP TABLE IF EXISTS seata_rs_at_resource_binding")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let source =
        MySqlDataSource::connect(owned_mysql::options(None).with_java_tc_mysql_db_lock_profile())
            .await
            .unwrap();
    let manager =
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap();
    let expected_key = manager.key().resource_id();
    let mut socket = h.accept().await;
    let client = h.rm.client();
    let cursor =
        SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'binding-check')")
            .unwrap();

    // 创建的严格事务和严格管理器分别可以要求验证，不能用同端点的旧调用绕过。
    let mut missing = source.begin().await.unwrap();
    assert_eq!(missing.execute(&cursor, &[]).await.unwrap(), 1);
    assert!(matches!(
        manager
            .prepare_transaction(missing, &client, "binding:missing-table", None)
            .await,
        Err(MySqlPrepareError::Local {
            source: DbError::Driver {
                code: Some(1146),
                ..
            },
        })
    ));
    sqlx::raw_sql(include_str!(
        "../../migrations/005_seata_rs_at_resource_binding.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    let mut reserved = h.source.begin().await.unwrap();
    let update = SqlRecognizerFactory::recognize(
        "UPDATE SEATA_RS_AT_RESOURCE_BINDING SET resource_id='alias' WHERE slot=1",
    )
    .unwrap();
    assert!(matches!(
        reserved.execute(&update, &[]).await,
        Err(DbError::Input {
            reason: "AT resource binding is reserved for administrative provisioning",
        })
    ));
    reserved.rollback().await.unwrap();

    for (name, origin, adapter) in [
        ("empty", &source, h.adapter.as_ref()),
        ("different", &h.source, &manager),
    ] {
        if name == "different" {
            sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
                .bind(expected_key.to_ascii_uppercase().into_bytes())
                .execute(&mut h.admin)
                .await
                .unwrap();
        }
        let mut tx = origin.begin().await.unwrap();
        assert_eq!(tx.execute(&cursor, &[]).await.unwrap(), 1);
        let failure = adapter
            .prepare_transaction(tx, &client, &format!("binding:{name}"), None)
            .await
            .unwrap_err();
        assert!(matches!(
            failure,
            MySqlPrepareError::Local {
                source: DbError::Input {
                    reason: "MySQL AT resource binding is missing or differs"
                },
            }
        ));
        assert!(failure.registered_branch().is_none());
        assert!(!failure.is_registration_unknown());
    }
    sqlx::query("UPDATE seata_rs_at_resource_binding SET resource_id=? WHERE slot=1")
        .bind(expected_key.as_bytes())
        .execute(&mut h.admin)
        .await
        .unwrap();
    let mut tx = source.begin().await.unwrap();
    assert_eq!(tx.execute(&cursor, &[]).await.unwrap(), 1);
    let reply = async {
        let request = rm_fixture::peer::read(&mut socket).await;
        let Some(Message::BranchRegisterRequest(body)) = request.body else {
            panic!("expected the first registration request");
        };
        assert_eq!(body.xid.as_deref(), Some("binding:authorized"));
        assert_eq!(body.resource_id.as_deref(), Some(expected_key));
        first_fixture::positive(&mut socket, request.id).await;
    };
    let (prepared, ()) = tokio::join!(
        manager.prepare_transaction(tx, &client, "binding:authorized", None),
        reply
    );
    let prepared = prepared.unwrap();
    assert_eq!(
        prepared.registered_branch().unwrap().identity().branch_id(),
        42
    );
    // 登记后同一业务事务仍持有共享行锁，管理员写入不能跨过准备判定。
    sqlx::query("SET SESSION innodb_lock_wait_timeout=1")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let blocked =
        sqlx::query("UPDATE seata_rs_at_resource_binding SET resource_id=X'00' WHERE slot=1")
            .execute(&mut h.admin)
            .await
            .unwrap_err();
    assert_eq!(
        blocked
            .as_database_error()
            .unwrap()
            .downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .number(),
        1205
    );
    sqlx::query("SET SESSION lock_wait_timeout=1")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let ddl = "ALTER TABLE seata_rs_at_resource_binding COMMENT='binding fixture'";
    let blocked_ddl = sqlx::query(ddl).execute(&mut h.admin).await.unwrap_err();
    assert_eq!(
        blocked_ddl
            .as_database_error()
            .unwrap()
            .downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .number(),
        1205
    );
    prepared.rollback().await.unwrap();
    sqlx::query(ddl).execute(&mut h.admin).await.unwrap();
    assert_eq!(
        sqlx::query("UPDATE seata_rs_at_resource_binding SET resource_id=X'00' WHERE slot=1")
            .execute(&mut h.admin)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
    source.close().await.unwrap();
    fixture::save(
        "resource-binding-guard.json",
        &serde_json::json!({
            "rejected_without_requests": 3,
            "registered_requests": 1,
            "resource_binding_held_until_rollback": true,
            "ddl_blocked_until_rollback": true,
            "business_and_undo_rows": 0
        }),
    );
}
