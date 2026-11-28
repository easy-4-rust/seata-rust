//! 固定原 Java TC 对业务主键排序规则等价的锁覆盖验证。
use super::{fixture, lock_guard_cases::checkpoint};
use seata_at::SqlRecognizerFactory;
use seata_datasource::FieldValue;
use seata_db_mysql::{DbError, MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::transaction::GlobalLockQueryRequest;
use seata_protocol::{BranchType, GlobalStatus, Message};
use seata_rm::{BranchResource, ResourceManager, RmCallError};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use serde_json::json;
use sqlx::MySqlConnection;
use std::{path::Path, time::Duration};
use tokio::time::{sleep, Instant};

pub(super) async fn exercise(
    admin: &mut MySqlConnection,
    source: &MySqlDataSource,
    manager: &MySqlDataSourceManager,
    rm: &ResourceManager,
    tm: &TransactionManager,
    folder: &Path,
    enforce: bool,
) {
    for (table, collation, original, alternate) in [
        ("storage_pad", "utf8mb4_general_ci", "key", "key "),
        ("storage_match", "utf8mb4_0900_ai_ci", "Cafe", "café"),
    ] {
        sqlx::query(&format!(
            "CREATE TABLE {table}(id VARCHAR(32) CHARACTER SET utf8mb4 COLLATE {collation} PRIMARY KEY) ENGINE=InnoDB"
        ))
        .execute(&mut *admin).await.unwrap();
        let insert = format!("INSERT INTO {table}(id) VALUES(?)");
        sqlx::query(&insert)
            .bind(original)
            .execute(&mut *admin)
            .await
            .unwrap();
        let error = sqlx::query(&insert)
            .bind(alternate)
            .execute(&mut *admin)
            .await
            .unwrap_err();
        assert_eq!(
            error
                .as_database_error()
                .unwrap()
                .downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
                .number(),
            1062
        );
        let found: Vec<u8> = sqlx::query_scalar(&format!(
            "SELECT CAST(id AS BINARY) FROM {table} WHERE id=?"
        ))
        .bind(alternate)
        .fetch_one(&mut *admin)
        .await
        .unwrap();
        assert_eq!(found, original.as_bytes());
    }

    let table_case_mode: u64 = sqlx::query_scalar("SELECT @@lower_case_table_names")
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    let client = rm.client();
    let config = TransactionConfig::builder("native-collation-equivalence".into())
        .transaction_timeout(Duration::from_secs(90))
        .build()
        .unwrap();
    let resource = manager.key().resource_id();
    let mut globals = Vec::new();
    let mut observations = Vec::new();
    for (name, table, pk, owner, query) in [
        ("pad-owner", "storage_pad", "key", true, None),
        (
            "pad-contender",
            "storage_pad",
            "key ",
            false,
            (!enforce).then_some(true),
        ),
        ("match-owner", "storage_match", "Cafe", true, None),
        (
            "match-contender",
            "storage_match",
            "café",
            false,
            Some(false),
        ),
    ] {
        let global = tm.begin(&config).await.unwrap();
        let xid = global.xid().as_str();
        let sql_table = if table_case_mode != 0 && name == "match-contender" {
            "StOrAgE_MaTcH"
        } else {
            table
        };
        let lock_key = format!("{sql_table}:{pk}");
        if let Some(expected) = query {
            let result = client
                .lock_query(GlobalLockQueryRequest {
                    xid: Some(xid.into()),
                    branch_type: BranchType::At,
                    resource_id: Some(resource.into()),
                    lock_key: Some(lock_key.clone()),
                    application_data: None,
                })
                .await
                .unwrap();
            assert_eq!(result.lockable, expected);
        }
        let sql = if owner {
            format!("DELETE FROM {table} WHERE id=?")
        } else {
            format!("INSERT INTO {sql_table}(id) VALUES(?)")
        };
        let recognized = SqlRecognizerFactory::recognize(&sql).unwrap();
        let mut local = source.begin().await.unwrap();
        assert_eq!(
            local
                .execute(&recognized, &[FieldValue::Text(pk.into())])
                .await
                .unwrap(),
            1
        );
        let result = manager.prepare_transaction(local, &client, xid, None).await;
        let rejected_locally = enforce && table == "storage_pad";
        let (branch_id, exception) = match result {
            Err(MySqlPrepareError::Local { source: DbError::Unsupported {
                feature: "business text primary key collation differs from Java TC MySQL lock storage",
            } }) if rejected_locally => (None, None),
            Err(MySqlPrepareError::Registration(RmCallError::DefiniteRejected { response, .. }))
                if name == "match-contender" => {
                let Some(Message::BranchRegisterResponse(reply)) = response.body else {
                    panic!("expected the original branch register response")
                };
                (None, Some(reply.response.transaction_exception_code.to_java_v1()))
            }
            Ok(prepared) if !rejected_locally && name != "match-contender" => {
                let receipt = prepared.registered_branch().unwrap();
                assert_eq!(receipt.lock_key(), Some(lock_key.as_str()));
                let branch = receipt.identity().branch_id();
                prepared.rollback().await.unwrap();
                (Some(branch), None)
            }
            result => panic!("unexpected registration result for {name}: {result:?}"),
        };
        if owner {
            // 夹具独立移除原行，模拟阶段一已释放本地行锁，TC 持锁保持原现场链路。
            let removed = sqlx::query(&format!("DELETE FROM {table} WHERE id=?"))
                .bind(pk)
                .execute(&mut *admin)
                .await
                .unwrap()
                .rows_affected();
            assert_eq!(removed, 1);
        }
        let observed = json!({"case":name, "xid":xid, "resource":resource, "table":table, "pk":pk,
            "lock_key":lock_key, "branch_id":branch_id, "exception":exception,
            "locally_rejected":rejected_locally, "lockable":query});
        checkpoint(folder, name, &observed).await;
        observations.push(observed);
        globals.push(global);
    }
    for global in &mut globals {
        assert!(matches!(
            global.rollback().await.unwrap(),
            TransactionCompletion::Observed { .. }
        ));
    }
    for global in &globals {
        let deadline = Instant::now() + Duration::from_secs(8);
        while tm.get_status(global.xid()).await.unwrap().status() != GlobalStatus::Finished {
            assert!(
                Instant::now() < deadline,
                "original global transaction did not finish"
            );
            sleep(Duration::from_millis(50)).await;
        }
    }
    for table in ["storage_pad", "storage_match"] {
        assert_eq!(fixture::count(admin, table).await, 0);
    }
    fixture::save("tc-lock-guard-result.json", &json!({"cases":observations}));
}
