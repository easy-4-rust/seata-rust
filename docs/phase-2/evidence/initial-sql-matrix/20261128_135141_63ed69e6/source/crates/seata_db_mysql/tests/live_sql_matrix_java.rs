//! 首期 SQL 支持矩阵：正式准备、原子 checksum、原 TC 提交及补偿。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_at::SqlRecognizerFactory;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;

fn matrix() -> serde_json::Value {
    serde_json::from_str(include_str!("data/mysql_sql_matrix.json")).unwrap()
}

#[test]
fn initial_sql_matrix_is_classified_without_broadening_unsupported_syntax() {
    let data = matrix();
    assert_eq!(data["accepted"].as_array().unwrap().len(), 22);
    for case in data["accepted"].as_array().unwrap() {
        assert!(
            SqlRecognizerFactory::recognize(case["sql"].as_str().unwrap()).is_ok(),
            "{}",
            case["id"]
        );
        assert_ne!(case["expected"], data["seed"]);
    }
    assert_eq!(data["rejected"].as_array().unwrap().len(), 22);
    for sql in data["rejected"].as_array().unwrap() {
        assert!(
            SqlRecognizerFactory::recognize(sql.as_str().unwrap()).is_err(),
            "{sql}"
        );
    }
}

#[cfg(feature = "experimental_changed_commit")]
mod native {
    use super::{matrix, owned_mysql};
    use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
    use seata_datasource::FieldValue;
    use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
    use seata_protocol::{v1::FrameLimits, GlobalStatus, PayloadLimits};
    use seata_rm::{
        BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager,
    };
    use seata_tm::{TransactionConfig, TransactionManager};
    use seata_transport::{
        ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
    };
    use sha2::{Digest, Sha256};
    use sqlx::Row;
    use std::{
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    };
    use tokio::time::{sleep, Instant};

    fn pool(endpoint: &str) -> ConnectionPoolConfig {
        ConnectionPoolConfig::new(
            endpoint.parse().unwrap(),
            ConnectionConfig::new(
                FrameLimits::new(
                    65536,
                    32767,
                    128,
                    PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
                )
                .unwrap(),
                64,
                Duration::from_secs(10),
                Duration::from_secs(10),
            )
            .unwrap(),
            1,
            64,
            ReconnectPolicy::new(
                Duration::from_millis(50),
                Duration::from_millis(200),
                Duration::from_secs(1),
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn save(out: &Path, name: &str, value: serde_json::Value) {
        std::fs::write(out.join(name), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
    async fn signal(out: &Path, name: &str) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !out.join(name).exists() {
            assert!(
                Instant::now() < deadline,
                "missing matrix observation {name}"
            );
            sleep(Duration::from_millis(20)).await;
        }
    }
    async fn rows(admin: &mut sqlx::MySqlConnection) -> serde_json::Value {
        let data = sqlx::query("SELECT id,v,note FROM sql_matrix_records ORDER BY id")
            .fetch_all(admin)
            .await
            .unwrap();
        serde_json::json!(data
            .iter()
            .map(|r| serde_json::json!([
                r.get::<i32, _>(0),
                r.get::<Option<i32>, _>(1),
                r.get::<Option<String>, _>(2)
            ]))
            .collect::<Vec<_>>())
    }

    #[tokio::test]
    #[ignore = "requires owned original Java TC and MySQL; run native_sql_matrix.py"]
    async fn original_java_tc_executes_initial_sql_matrix() {
        tokio::time::timeout(Duration::from_secs(600), run())
            .await
            .unwrap();
    }
    async fn run() {
        let out = PathBuf::from(std::env::var("SEATA_SQL_MATRIX_OUT").unwrap());
        let commit = std::env::var("SEATA_SQL_MATRIX_DIRECTION").unwrap() == "commit";
        let mut admin = owned_mysql::admin().await;
        let source = MySqlDataSource::connect(
            owned_mysql::options(None)
                .with_java_tc_mysql_db_lock_profile()
                .require_at_jdbc_profile_binding()
                .operation_timeout(Duration::from_secs(15))
                .unwrap(),
        )
        .await
        .unwrap();
        let manager = Arc::new(
            MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default)
                .unwrap(),
        );
        let catalog = ResourceCatalog::new(4, 32767).unwrap();
        catalog.register(manager.clone()).unwrap();
        let journal = FileBranchStore::open(
            out.join("branches.log"),
            JournalOptions::new(128, 8388608, 65536, 128).unwrap(),
        )
        .await
        .unwrap();
        let identity = RegistrationIdentity::new(
            "1.1.0".into(),
            "sql-matrix-rust".into(),
            "cross_lock".into(),
            None,
        )
        .unwrap();
        let rm = ResourceManager::builder(
            pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
            identity.clone(),
            catalog,
            journal,
        )
        .request_timeout(Duration::from_secs(15))
        .callback_timeout(Duration::from_secs(20))
        .build()
        .unwrap();
        let tm = TransactionManager::builder(
            pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
            identity,
        )
        .request_timeout(Duration::from_secs(20))
        .build()
        .unwrap();
        let cfg = TransactionConfig::builder("initial-sql-matrix".into())
            .transaction_timeout(Duration::from_secs(90))
            .build()
            .unwrap();
        let data = matrix();
        for case in data["accepted"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            sqlx::raw_sql("DROP TABLE IF EXISTS sql_matrix_records; CREATE TABLE sql_matrix_records(id INT PRIMARY KEY,v INT NULL,note VARCHAR(64) NULL) ENGINE=InnoDB; INSERT INTO sql_matrix_records VALUES(1,12,'one'),(2,20,'two'),(3,NULL,NULL),(4,40,'four')")
                .execute(&mut admin).await.unwrap();
            assert_eq!(rows(&mut admin).await, data["seed"]);
            let mut global = tm.begin(&cfg).await.unwrap();
            let xid = global.xid().clone();
            let params: Vec<FieldValue> = case["params"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    if p.is_null() {
                        FieldValue::Null
                    } else if let Some(n) = p.as_i64() {
                        FieldValue::Signed(n)
                    } else {
                        FieldValue::Text(p.as_str().unwrap().into())
                    }
                })
                .collect();
            let mut local = source.begin().await.unwrap();
            let affected = local
                .execute(
                    &SqlRecognizerFactory::recognize(case["sql"].as_str().unwrap()).unwrap(),
                    &params,
                )
                .await
                .unwrap();
            assert_eq!(
                rows(&mut admin).await,
                data["seed"],
                "uncommitted {id} leaked"
            );
            let checked = manager
                .prepare_transaction(local, &rm.client(), xid.as_str(), None)
                .await
                .unwrap()
                .lock_commit_preconditions()
                .await
                .unwrap();
            let receipt = checked
                .commit_changed_and_report(&manager, &rm.client())
                .await
                .unwrap();
            let branch = receipt.identity().branch_id();
            assert_eq!(rows(&mut admin).await, case["expected"], "phase one {id}");
            let witness: (Vec<u8>, Vec<u8>, u8, u8) = sqlx::query_as("SELECT u.rollback_info,b.undo_sha256,b.first_phase_committed,b.jdbc_profile FROM undo_log u JOIN seata_rs_at_branch_v1 b ON b.xid=CAST(u.xid AS BINARY) AND b.branch_id=u.branch_id WHERE u.xid=? AND u.branch_id=? AND u.log_status=0")
                .bind(xid.as_str()).bind(branch).fetch_one(&mut admin).await.unwrap();
            assert_eq!(Sha256::digest(&witness.0).to_vec(), witness.1);
            assert_eq!((witness.2, witness.3), (1, 1));
            std::fs::write(out.join(format!("{id}.undo.json")), &witness.0).unwrap();
            save(
                &out,
                &format!("{id}.held.json"),
                serde_json::json!({"id":id,"xid":xid.as_str(),"branch_id":branch,
                "resource":manager.key().resource_id(),"lock":receipt.lock_key(),"affected_rows":affected,
                "phase_one":rows(&mut admin).await,"checksum_persisted":true}),
            );
            signal(&out, &format!("{id}.release")).await;
            if commit {
                global.commit().await.unwrap();
            } else {
                global.rollback().await.unwrap();
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
                assert!(Instant::now() < deadline);
                sleep(Duration::from_millis(20)).await;
            }
            assert_eq!(
                rows(&mut admin).await,
                if commit {
                    &case["expected"]
                } else {
                    &data["seed"]
                }
                .clone(),
                "final {id}"
            );
            save(
                &out,
                &format!("{id}.finished.json"),
                serde_json::json!({"id":id,"xid":xid.as_str(),"branch_id":branch,"rows":rows(&mut admin).await}),
            );
            signal(&out, &format!("{id}.ack")).await;
        }
        tm.shutdown().await.unwrap();
        rm.shutdown().await.unwrap();
        source.close().await.unwrap();
    }
}
