//! 真实 SQLx 解析被受控改写的影响行响应；检查整笔本地事务拒绝与原 TC 零分支。
use crate::{fixture, owned_mysql, pool, registration};
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_datasource::ImageError;
use seata_db_mysql::{DbError, MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::GlobalStatus;
use seata_rm::{BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use std::{path::PathBuf, sync::Arc, time::Duration};

#[tokio::test]
#[ignore = "requires original Java TC and owned MySQL affected-row packet fault"]
async fn original_java_tc_rejects_native_affected_count_without_images() {
    let mut admin = fixture::reset().await;
    for migration in [
        include_str!("../../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../../migrations/005_seata_rs_at_resource_binding.sql"),
        include_str!("../../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    let port = std::env::var("SEATA_JAVA_RM_MYSQL_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let source = MySqlDataSource::connect(
        owned_mysql::options(Some(port)).with_java_tc_mysql_db_lock_profile(),
    )
    .await
    .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(adapter.key().resource_id().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let store = FileBranchStore::open(
        folder.join("java-affected-image-branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        store,
    )
    .request_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let tm = TransactionManager::builder(
        pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
        registration(),
    )
    .request_timeout(Duration::from_secs(10))
    .build()
    .unwrap();
    let cfg = TransactionConfig::builder("native-affected-image-guard".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut cases = Vec::new();
    for (kind, prior) in [
        ("update", false),
        ("delete", false),
        ("update", true),
        ("delete", true),
    ] {
        let mut global = tm.begin(&cfg).await.unwrap();
        let xid = global.xid().clone();
        let mut local = source.begin().await.unwrap();
        if prior {
            assert_eq!(
                local
                    .execute(
                        &SqlRecognizerFactory::recognize(
                            "INSERT INTO undo_work(id,v) VALUES(1,'prior-uncommitted')"
                        )
                        .unwrap(),
                        &[]
                    )
                    .await
                    .unwrap(),
                1
            );
        }
        let sql = if kind == "update" {
            "UPDATE undo_work SET v='missing' WHERE id=99"
        } else {
            "DELETE FROM undo_work WHERE id=99"
        };
        let error = local
            .execute(&SqlRecognizerFactory::recognize(sql).unwrap(), &[])
            .await
            .expect_err("native affected count without any image must be rejected");
        assert!(
            matches!(
                error,
                DbError::Image(ImageError::Invalid {
                    reason: "affected business rows require Undo image"
                })
            ),
            "{error:?}"
        );
        assert_eq!(local.undo_logs().len(), usize::from(prior));
        assert!(matches!(
            local
                .execute(
                    &SqlRecognizerFactory::recognize(
                        "INSERT INTO undo_work(id,v) VALUES(2,'forbidden')"
                    )
                    .unwrap(),
                    &[]
                )
                .await,
            Err(DbError::Poisoned)
        ));
        let error = adapter
            .prepare_transaction(local, &rm.client(), xid.as_str(), None)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                MySqlPrepareError::Local {
                    source: DbError::Poisoned
                }
            ),
            "{error:?}"
        );
        let observed_end = match global.rollback().await.unwrap() {
            TransactionCompletion::Observed { outcome, .. } => outcome.status().to_java_v1(),
            other => panic!("unexpected original global rollback result: {other:?}"),
        };
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut status_queries = Vec::new();
        loop {
            let status = tm.get_status(&xid).await.unwrap().status();
            status_queries.push(status.to_java_v1());
            if status == GlobalStatus::Finished {
                break;
            }
            assert!(matches!(
                status,
                GlobalStatus::Rollbacking | GlobalStatus::Rollbacked
            ));
            assert!(
                tokio::time::Instant::now() < deadline,
                "original TC cleanup required"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        cases.push(serde_json::json!({"kind":kind,"prior":prior,"xid":xid.as_str(),"image_error":true,"poisoned_execute":true,"poisoned_prepare":true,"global_finished":true,"observed_end":observed_end,"status_queries":status_queries}));
    }
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    source.close().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    fixture::save(
        "java-affected-image-ended.json",
        &serde_json::json!({"cases":cases,"business_rows":0,"undo_rows":0}),
    );
}
