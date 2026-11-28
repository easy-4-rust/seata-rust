//! 首期 SQL 支持矩阵：正式准备、原子 checksum、原 TC 提交及补偿。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(feature = "experimental_changed_commit")]
#[path = "../../seata_orm_rbatis/tests/support/entity.rs"]
mod entity_cases;
#[cfg(feature = "experimental_changed_commit")]
#[path = "../../seata_orm_rbatis/tests/support/owned_mysql.rs"]
mod owned_mysql;
#[cfg(feature = "experimental_changed_commit")]
#[path = "../../seata_orm_rbatis/tests/support/read_checks.rs"]
mod rbatis_read_checks;
#[cfg(feature = "experimental_changed_commit")]
fn matrix() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../seata_orm_rbatis/tests/data/entity_cases.json"
    ))
    .unwrap()
}
#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
async fn rbatis_generated_writes_are_classified_without_interpolating_bindings() {
    let data = matrix();
    assert_eq!(data["accepted"].as_array().unwrap().len(), 3);
    for case in data["accepted"].as_array().unwrap() {
        let plan = entity_cases::plan(case["id"].as_str().unwrap()).await;
        assert_eq!(plan.recognizer().table_name(), "orm_items");
        assert!(!plan.recognizer().sql().contains("new ?"));
    }
}
#[cfg(feature = "experimental_changed_commit")]
mod native {
    use super::{entity_cases, matrix, owned_mysql};
    use seata_at::MySqlJdbcProfile;
    use seata_client::SeataClient;
    use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
    use seata_orm_rbatis::RbatisMySqlExecutor;
    use seata_protocol::{v1::FrameLimits, GlobalStatus, PayloadLimits};
    use seata_rm::{BranchResource, FileBranchStore, JournalOptions, ResourceCatalog};
    use seata_tm::TransactionConfig;
    use seata_transport::{
        ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
    };
    use sha2::{Digest, Sha256};
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
        let data:Vec<String>=sqlx::query_scalar("SELECT CAST(JSON_ARRAY(CAST(id AS CHAR),v,note,CAST(amount AS CHAR),HEX(bytes),payload) AS CHAR) FROM orm_items ORDER BY id").fetch_all(admin).await.unwrap();
        serde_json::Value::Array(
            data.into_iter()
                .map(|v| serde_json::from_str(&v).unwrap())
                .collect(),
        )
    }

    #[tokio::test]
    #[ignore = "requires owned original Java TC and MySQL; run native_client_mysql.py"]
    async fn original_java_tc_executes_unified_client() {
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
            "rbatis-entity-rust".into(),
            "cross_lock".into(),
            None,
        )
        .unwrap();
        assert_eq!(
            std::env::var("SEATA_RM_ENDPOINT").unwrap(),
            std::env::var("SEATA_TM_ENDPOINT").unwrap()
        );
        let client =
            SeataClient::builder(pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()), identity)
                .enable_tm()
                .enable_rm(catalog, journal)
                .allow_plaintext()
                .request_timeout(Duration::from_secs(20))
                .callback_timeout(Duration::from_secs(20))
                .build()
                .await
                .unwrap();
        let rm = client.resource_manager().unwrap();
        let tm = client.transaction_manager().unwrap();
        let cfg = TransactionConfig::builder("rbatis-entity-mysql".into())
            .transaction_timeout(Duration::from_secs(90))
            .build()
            .unwrap();
        super::rbatis_read_checks::run(&source, &mut admin, &out).await;
        let data = matrix();
        for case in data["accepted"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            sqlx::raw_sql("DROP TABLE IF EXISTS orm_items; CREATE TABLE orm_items(id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,v INT NULL,note VARCHAR(64) NULL,amount DECIMAL(24,4) NOT NULL,bytes VARBINARY(16) NOT NULL,payload JSON NOT NULL) ENGINE=InnoDB AUTO_INCREMENT=100; INSERT INTO orm_items VALUES(1,12,'one',12.3400,X'0100FF','{\"a\":1}'),(2,NULL,NULL,0.0000,X'','null')")
                .execute(&mut admin).await.unwrap();
            assert_eq!(rows(&mut admin).await, data["seed"]);
            let mut global = tm.begin(&cfg).await.unwrap();
            let xid = global.xid().clone();
            let executor = RbatisMySqlExecutor::new(source.begin().await.unwrap());
            let (affected, model) = entity_cases::apply(id, &executor).await;
            let local = executor.into_transaction();
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
                "resource":manager.key().resource_id(),"lock":receipt.lock_key(),"affected_rows":affected,"native_model":model,
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
        let tm_sampler = tm.connection_sampler();
        let rm_sampler = rm.connection_sampler();
        client.shutdown().await.unwrap();
        assert!(tm_sampler.snapshot().closed && rm_sampler.snapshot().closed);
        save(
            &out,
            "client-lifecycle.json",
            serde_json::json!({"same_endpoint":true,"roles":["tm","rm"],"tm_closed":true,"rm_closed":true}),
        );
        source.close().await.unwrap();
    }
}
