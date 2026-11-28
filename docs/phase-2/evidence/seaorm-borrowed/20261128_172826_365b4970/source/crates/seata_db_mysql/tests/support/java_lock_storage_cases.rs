//! 固定原 Java TC 的 DB 锁字段长度与排序规则夹具。
use super::{fixture, lock_guard_cases::checkpoint};
use seata_at::{LockKeyError, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::transaction::GlobalLockQueryRequest;
use seata_protocol::{BranchType, GlobalStatus, Message};
use seata_rm::{BranchResource, ResourceManager, RmCallError};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use serde_json::json;
use sqlx::MySqlConnection;
use std::{path::Path, time::Duration};
use tokio::time::{sleep, Instant};

const SHORT_TABLE: &str = "storage_text";

pub(super) async fn exercise(
    admin: &mut MySqlConnection,
    source: &MySqlDataSource,
    manager: &MySqlDataSourceManager,
    rm: &ResourceManager,
    tm: &TransactionManager,
    folder: &Path,
    enforce_lock_limits: bool,
) {
    let table_32 = "t".repeat(32);
    let table_33 = "t".repeat(33);
    for table in [SHORT_TABLE, &table_32, &table_33] {
        sqlx::query(&format!(
            "CREATE TABLE {table}(id VARCHAR(80) PRIMARY KEY) ENGINE=InnoDB COLLATE=utf8mb4_0900_as_cs"
        ))
        .execute(&mut *admin)
        .await
        .unwrap();
    }

    // 业务主键允许同时写入这两个值，TC 锁表随后使用自己的排序规则比较。
    let insert = SqlRecognizerFactory::recognize(&format!(
        "INSERT INTO {SHORT_TABLE}(id) VALUES('Cafe'),('café')"
    ))
    .unwrap();
    let mut pair = source.begin().await.unwrap();
    assert_eq!(pair.execute(&insert, &[]).await.unwrap(), 2);
    pair.rollback().await.unwrap();
    let resource = manager.key().resource_id();
    let row_capacity = 128 - resource.chars().count() - table_32.len() - 6;
    assert!((1..36).contains(&row_capacity));
    let cases = [
        ("pk-36", SHORT_TABLE, "🦀".repeat(36), true),
        ("pk-37", SHORT_TABLE, "🦀".repeat(37), false),
        ("table-32", table_32.as_str(), "table".into(), true),
        ("table-33", table_33.as_str(), "table".into(), false),
        ("row-128", table_32.as_str(), "r".repeat(row_capacity), true),
        (
            "row-129",
            table_32.as_str(),
            "r".repeat(row_capacity + 1),
            false,
        ),
        ("collation-owner", SHORT_TABLE, "Cafe".into(), true),
        ("collation-contender", SHORT_TABLE, "café".into(), false),
    ];
    let config = TransactionConfig::builder("native-lock-storage".into())
        .transaction_timeout(Duration::from_secs(90))
        .build()
        .unwrap();
    let client = rm.client();
    let mut pending = Vec::new();
    let mut observations = Vec::new();
    for (name, table, pk, accepted) in cases {
        let transaction = tm.begin(&config).await.unwrap();
        let xid = transaction.xid().clone();
        let lock_key = format!("{table}:{pk}");
        if name == "collation-contender" {
            let lock = client
                .lock_query(GlobalLockQueryRequest {
                    xid: Some(xid.as_str().into()),
                    branch_type: BranchType::At,
                    resource_id: Some(resource.into()),
                    lock_key: Some(lock_key.clone()),
                    application_data: None,
                })
                .await
                .unwrap();
            assert!(!lock.lockable);
        }
        let insert =
            SqlRecognizerFactory::recognize(&format!("INSERT INTO {table}(id) VALUES(?)")).unwrap();
        let mut local = source.begin().await.unwrap();
        assert_eq!(
            local
                .execute(&insert, &[FieldValue::Text(pk.clone())])
                .await
                .unwrap(),
            1
        );
        let result = manager
            .prepare_transaction(local, &client, xid.as_str(), None)
            .await;
        let local_limit = if enforce_lock_limits {
            match name {
                "pk-37" => Some("Java TC MySQL lock_table.pk (36 characters)"),
                "table-33" => Some("Java TC MySQL lock_table.table_name (32 characters)"),
                "row-129" => Some("Java TC MySQL lock_table.row_key (128 characters)"),
                _ => None,
            }
        } else {
            None
        };
        let (branch_id, exception) = match result {
            Err(MySqlPrepareError::LockKey(LockKeyError::Budget { limit }))
                if local_limit == Some(limit) =>
            {
                (None, None)
            }
            Ok(prepared) if accepted => {
                let receipt = prepared.registered_branch().unwrap();
                assert_eq!(receipt.lock_key(), Some(lock_key.as_str()));
                let branch = receipt.identity().branch_id();
                prepared.rollback().await.unwrap();
                (Some(branch), None)
            }
            Err(MySqlPrepareError::Registration(RmCallError::DefiniteRejected {
                response,
                ..
            })) if !accepted && local_limit.is_none() => {
                let Some(Message::BranchRegisterResponse(rejected)) = response.body else {
                    panic!("registration response family differs");
                };
                (
                    None,
                    Some(rejected.response.transaction_exception_code.to_java_v1()),
                )
            }
            result => panic!("unexpected registration result for {name}: {result:?}"),
        };
        let observed = json!({"case":name,"xid":xid.as_str(),"resource":resource,"table":table,"pk":pk,
            "lock_key":lock_key,"branch_id":branch_id,"exception":exception});
        let mut observed = observed;
        if enforce_lock_limits {
            observed["local_limit"] = json!(local_limit);
        }
        checkpoint(folder, name, &observed).await;
        observations.push(observed);
        pending.push(transaction);
    }
    for transaction in &mut pending {
        assert!(matches!(
            transaction.rollback().await.unwrap(),
            TransactionCompletion::Observed { .. }
        ));
    }
    for transaction in &pending {
        let deadline = Instant::now() + Duration::from_secs(8);
        while tm.get_status(transaction.xid()).await.unwrap().status() != GlobalStatus::Finished {
            assert!(Instant::now() < deadline, "TC did not finish transaction");
            sleep(Duration::from_millis(50)).await;
        }
    }
    for table in [SHORT_TABLE, &table_32, &table_33] {
        assert_eq!(fixture::count(admin, table).await, 0);
    }
    fixture::save("tc-lock-guard-result.json", &json!({"cases":observations}));
}
