//! 固定原 Java TC 的资源别名和复合主键身份夹具。
use super::{fixture, lock_guard_cases::checkpoint, owned_mysql};
use seata_at::{LockKeyError, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use seata_db_mysql::{DbError, MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::transaction::GlobalLockQueryRequest;
use seata_protocol::{BranchType, GlobalStatus, Message, TransactionExceptionCode};
use seata_rm::{BranchResource, ResourceManager, RmCallError};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use serde_json::json;
use sqlx::MySqlConnection;
use std::{path::Path, time::Duration};
use tokio::time::{sleep, Instant};

const COMPOSITE: &str = "identity_pair";
const SAME_ROW: &str = "undo_work:1";

pub(super) async fn exercise(
    admin: &mut MySqlConnection,
    primary: (&MySqlDataSource, &MySqlDataSourceManager),
    alias: (u16, &MySqlDataSource, &MySqlDataSourceManager),
    rm: &ResourceManager,
    tm: &TransactionManager,
    folder: &Path,
    binding_profile: (bool, bool),
) {
    let (require_binding, combined_profile) = binding_profile;
    let (alias_port, alias_source, alias_manager) = alias;
    let alias = (alias_source, alias_manager);
    sqlx::query("INSERT INTO undo_work(id,v) VALUES(1,'physical-seed')")
        .execute(&mut *admin)
        .await
        .unwrap();
    let mut alias_observer = owned_mysql::admin_at(alias_port).await;
    let provenance =
        "SELECT @@server_uuid,DATABASE(),CAST(@@port AS CHAR),v FROM undo_work WHERE id=1";
    let physical: (String, String, String, String) = sqlx::query_as(provenance)
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    let alternate: (String, String, String, String) = sqlx::query_as(provenance)
        .fetch_one(&mut alias_observer)
        .await
        .unwrap();
    assert_eq!(physical, alternate);
    assert_eq!(physical.3, "physical-seed");
    assert_ne!(primary.1.key().resource_id(), alias.1.key().resource_id());
    // 组合配置的正向夹具必须与原 TC 锁表使用同一排序规则；历史诊断维持原 as_cs。
    let collation = if combined_profile {
        "utf8mb4_0900_ai_ci"
    } else {
        "utf8mb4_0900_as_cs"
    };
    sqlx::query(&format!("CREATE TABLE identity_pair(a VARCHAR(16),b VARCHAR(16),PRIMARY KEY(a,b)) ENGINE=InnoDB COLLATE={collation}"))
        .execute(&mut *admin)
        .await
        .unwrap();

    // 原业务库可同时存放三个不同元组，其中两组在原 TC 协议上拼接成同一个锁键。
    let insert_all = SqlRecognizerFactory::recognize(
        "INSERT INTO identity_pair(a,b) VALUES('a_b','c'),('a','b_c'),('a','bd')",
    )
    .unwrap();
    let mut coexist = primary.0.begin().await.unwrap();
    assert_eq!(coexist.execute(&insert_all, &[]).await.unwrap(), 3);
    coexist.rollback().await.unwrap();
    let physical = json!({"server_uuid":physical.0,"schema":physical.1,"mysql_port":physical.2,
        "seed":physical.3,"primary":primary.1.key().resource_id(),"alias":alias.1.key().resource_id(),
        "distinct_composite_rows":3});
    fixture::save("identity-physical.json", &physical);
    let config = TransactionConfig::builder("native-lock-identity".into())
        .transaction_timeout(Duration::from_secs(90))
        .build()
        .unwrap();
    let cases = [
        ("alias-owner", primary, None, SAME_ROW, None),
        ("alias-same-id", primary, None, SAME_ROW, Some(false)),
        (
            "alias-other-id",
            alias,
            None,
            SAME_ROW,
            (!require_binding).then_some(true),
        ),
        (
            "composite-owner",
            primary,
            Some(("a_b", "c")),
            "identity_pair:a_b_c",
            None,
        ),
        (
            "composite-collision",
            primary,
            Some(("a", "b_c")),
            "identity_pair:a_b_c",
            None,
        ),
        (
            "composite-distinct",
            primary,
            Some(("a", "bd")),
            "identity_pair:a_bd",
            Some(true),
        ),
    ];
    let client = rm.client();
    let mut transactions = Vec::new();
    let mut observations = Vec::new();
    for (name, (source, manager), tuple, key, lockable) in cases {
        let transaction = tm.begin(&config).await.unwrap();
        let xid = transaction.xid().as_str();
        let resource = manager.key().resource_id();
        if let Some(expected) = lockable {
            let queried = client
                .lock_query(GlobalLockQueryRequest {
                    xid: Some(xid.into()),
                    branch_type: BranchType::At,
                    resource_id: Some(resource.into()),
                    lock_key: Some(key.into()),
                    application_data: None,
                })
                .await
                .unwrap();
            assert_eq!(queried.lockable, expected);
        }
        let mut local = source.begin().await.unwrap();
        let (statement, parameters, table) = if let Some((a, b)) = tuple {
            (
                "INSERT INTO identity_pair(a,b) VALUES(?,?)",
                vec![FieldValue::Text(a.into()), FieldValue::Text(b.into())],
                COMPOSITE,
            )
        } else {
            (
                "UPDATE undo_work SET v='local-only' WHERE id=1",
                Vec::new(),
                "undo_work",
            )
        };
        let cursor = SqlRecognizerFactory::recognize(statement).unwrap();
        assert_eq!(local.execute(&cursor, &parameters).await.unwrap(), 1);
        let result = manager.prepare_transaction(local, &client, xid, None).await;
        let alias_rejected = require_binding && name == "alias-other-id";
        let composite_rejected = matches!(name, "composite-owner" | "composite-collision");
        let (branch, exception) = if alias_rejected {
            let failure = result.unwrap_err();
            assert!(matches!(
                failure,
                MySqlPrepareError::Local {
                    source: DbError::Input {
                        reason: "MySQL AT resource binding is missing or differs"
                    },
                }
            ));
            assert!(!failure.is_registration_unknown());
            assert!(failure.registered_branch().is_none());
            (None, None)
        } else if composite_rejected {
            let failure = result.unwrap_err();
            assert!(matches!(
                failure,
                MySqlPrepareError::LockKey(LockKeyError::Unsupported {
                    feature: "ambiguous delimiter in composite primary key",
                })
            ));
            assert!(!failure.is_registration_unknown());
            assert!(failure.registered_branch().is_none());
            (None, None)
        } else if lockable == Some(false) {
            let Err(MySqlPrepareError::Registration(RmCallError::DefiniteRejected {
                response,
                ..
            })) = result
            else {
                panic!("expected original TC lock conflict for {name}: {result:?}");
            };
            let Some(Message::BranchRegisterResponse(rejected)) = response.body else {
                panic!("registration response family differs");
            };
            let exception = rejected.response.transaction_exception_code;
            assert!(matches!(
                exception,
                TransactionExceptionCode::LockKeyConflict
                    | TransactionExceptionCode::LockKeyConflictFailFast
            ));
            (None, Some(exception.to_java_v1()))
        } else {
            let prepared = result.unwrap();
            let receipt = prepared.registered_branch().unwrap();
            assert_eq!(receipt.lock_key(), Some(key));
            let branch = receipt.identity().branch_id();
            prepared.rollback().await.unwrap();
            (Some(branch), None)
        };
        let (_, pk) = key.split_once(':').unwrap();
        let observed = json!({"case":name,"xid":xid,"resource":resource,"table":table,"pk":pk,
            "lock_key":key,"branch_id":branch,"exception":exception,"lockable":lockable,"tuple":tuple,
            "local_rejected":alias_rejected || composite_rejected});
        checkpoint(folder, name, &observed).await;
        observations.push(observed);
        transactions.push(transaction);
    }
    for transaction in &mut transactions {
        assert!(matches!(
            transaction.rollback().await.unwrap(),
            TransactionCompletion::Observed { .. }
        ));
    }
    for transaction in &transactions {
        let deadline = Instant::now() + Duration::from_secs(8);
        while tm.get_status(transaction.xid()).await.unwrap().status() != GlobalStatus::Finished {
            assert!(Instant::now() < deadline, "TC did not finish transaction");
            sleep(Duration::from_millis(50)).await;
        }
    }
    let original: String = sqlx::query_scalar("SELECT v FROM undo_work WHERE id=1")
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    assert_eq!(original, "physical-seed");
    assert_eq!(fixture::count(admin, COMPOSITE).await, 0);
    sqlx::query("DELETE FROM undo_work WHERE id=1 AND v='physical-seed'")
        .execute(admin)
        .await
        .unwrap();
    fixture::save(
        "tc-lock-guard-result.json",
        &json!({"cases":observations,"physical":physical}),
    );
}
