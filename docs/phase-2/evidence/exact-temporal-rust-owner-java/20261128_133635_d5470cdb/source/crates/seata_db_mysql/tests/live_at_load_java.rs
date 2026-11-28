//! 正式 AT 多事务同时持有 Undo/全局锁的有限负载；对应 Java DataSourceProxy/ConnectionProxy。
#![cfg(feature = "experimental_changed_commit")]
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;

use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_protocol::{v1::FrameLimits, GlobalStatus, PayloadLimits};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResource, FileBranchStore,
    JournalOptions, ResourceCatalog, ResourceManager,
};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{Barrier, Mutex},
    task::JoinSet,
    time::Instant,
};

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

fn registration() -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some("phase0-local-only".into()),
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires owned original Java TC and MySQL; run at_concurrent_java.py"]
async fn original_java_tc_at_concurrent_load() {
    tokio::time::timeout(Duration::from_secs(180), run())
        .await
        .expect("bounded load run");
}

async fn run() {
    let concurrency: usize = std::env::var("SEATA_AT_LOAD_CONCURRENCY")
        .unwrap()
        .parse()
        .unwrap();
    assert!([1, 4, 8].contains(&concurrency));
    let rounds = 8;
    let out = PathBuf::from(std::env::var("SEATA_AT_LOAD_OUT").unwrap());
    let mut admin = owned_mysql::admin().await;
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert!(
        database.starts_with("undo_load_")
            && database == std::env::var("SEATA_MYSQL_DATABASE").unwrap()
    );
    for table in [
        "undo_work",
        "undo_log",
        "seata_rs_at_resource_binding",
        "seata_rs_at_commit_attempt",
        "seata_rs_at_branch_v1",
    ] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(&mut admin)
            .await
            .unwrap();
    }
    let ddl = std::fs::read_to_string(std::env::var("SEATA_UNDO_SCHEMA_PATH").unwrap()).unwrap();
    sqlx::raw_sql(&ddl).execute(&mut admin).await.unwrap();
    for migration in [
        include_str!("../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/005_seata_rs_at_resource_binding.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
        include_str!("../migrations/013_seata_rs_at_undo_checksum.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    sqlx::query("CREATE TABLE undo_work(id INT PRIMARY KEY,v VARCHAR(1000)) ENGINE=InnoDB")
        .execute(&mut admin)
        .await
        .unwrap();
    for id in 1..=concurrency * rounds {
        sqlx::query("INSERT INTO undo_work VALUES(?,'before'),(?,'delete_before')")
            .bind(id as i32)
            .bind(id as i32 + 20000)
            .execute(&mut admin)
            .await
            .unwrap();
    }
    let ds = MySqlDataSource::connect(
        owned_mysql::options(None)
            .with_java_tc_mysql_db_lock_profile()
            .pool_limits(16, Duration::from_secs(15))
            .unwrap()
            .operation_timeout(Duration::from_secs(30))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(ds.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let key = adapter.key().clone();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(key.resource_id().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let store = FileBranchStore::open(
        out.join("branches.log"),
        JournalOptions::new(256, 16777216, 65536, 32).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(15))
    .callback_timeout(Duration::from_secs(30))
    .build()
    .unwrap();
    let tm = Arc::new(
        TransactionManager::builder(
            pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
            registration(),
        )
        .request_timeout(Duration::from_secs(30))
        .build()
        .unwrap(),
    );
    let epoch = Instant::now();
    let mut results = Vec::new();
    for round in 0..rounds {
        let begun = Arc::new(Barrier::new(concurrency));
        let prepared = Arc::new(Barrier::new(concurrency + 1));
        let release = Arc::new(Barrier::new(concurrency + 1));
        let identities = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = JoinSet::new();
        for worker in 0..concurrency {
            let (tm, ds, adapter, client, begun, prepared, release, identities) = (
                tm.clone(),
                ds.clone(),
                adapter.clone(),
                rm.client(),
                begun.clone(),
                prepared.clone(),
                release.clone(),
                identities.clone(),
            );
            tasks.spawn(async move {
                let id = (round*concurrency+worker+1) as i32;
                let commit = id%2 == 0;
                let started_us = epoch.elapsed().as_micros() as u64;
                let cfg = TransactionConfig::builder("bounded-at-load".into()).transaction_timeout(Duration::from_secs(90)).build().unwrap();
                let mut global = tm.begin(&cfg).await.unwrap();
                let xid = global.xid().clone();
                begun.wait().await;
                let mut local = ds.begin().await.unwrap();
                for sql in [format!("UPDATE undo_work SET v='updated' WHERE id={id}"),
                    format!("INSERT INTO undo_work(id,v) VALUES({},'inserted')",id+10000),
                    format!("DELETE FROM undo_work WHERE id={}",id+20000)] {
                    local.execute(&SqlRecognizerFactory::recognize(&sql).unwrap(),&[]).await.unwrap();
                }
                let checked = adapter.prepare_transaction(local,&client,xid.as_str(),None).await.unwrap().lock_commit_preconditions().await.unwrap();
                let branch_id = checked.registered_branch().unwrap().identity().branch_id();
                checked.commit_changed_and_report(&adapter,&client).await.unwrap();
                let committed_us=epoch.elapsed().as_micros() as u64;
                identities.lock().await.push(serde_json::json!({"id":id,"xid":xid.as_str(),"branch_id":branch_id,"commit":commit,"started_us":started_us,"committed_us":committed_us}));
                prepared.wait().await;release.wait().await;
                let end_started_us = epoch.elapsed().as_micros() as u64;
                let completion=if commit {global.commit().await} else {global.rollback().await}.unwrap();
                let observed_status = match completion {
                    TransactionCompletion::Observed { outcome, .. } => outcome.status(),
                    other => panic!("{other:?}"),
                };
                assert!(if commit { observed_status == GlobalStatus::Committed } else { matches!(observed_status,GlobalStatus::Rollbacked|GlobalStatus::RollbackRetrying) });

                (id,xid,branch_id,commit,started_us,committed_us,end_started_us,observed_status)
            });
        }
        prepared.wait().await;
        let mut identities = identities.lock().await.clone();
        identities.sort_by_key(|v| v["id"].as_i64().unwrap());
        assert_eq!(identities.len(), concurrency);
        let mut snapshots = Vec::new();
        for item in &identities {
            let xid = item["xid"].as_str().unwrap();
            let branch = item["branch_id"].as_i64().unwrap();
            let row:(i64,i64,i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0),(SELECT COUNT(*) FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=? AND schema_version=4 AND first_phase_committed=1 AND commit_authorized=0 AND terminal=0 AND undo_sha256=UNHEX(SHA2((SELECT rollback_info FROM undo_log WHERE xid=? AND branch_id=?),256))),(SELECT COUNT(*) FROM seata_rs_at_commit_attempt WHERE xid=? AND branch_id=? AND report_started=1),(SELECT COUNT(*) FROM lock_table WHERE xid=? AND branch_id=?),(SELECT COUNT(*) FROM branch_table WHERE xid=? AND branch_id=?),(SELECT COUNT(*) FROM global_table WHERE xid=?)")
                .bind(xid).bind(branch).bind(xid.as_bytes()).bind(branch).bind(xid).bind(branch).bind(xid.as_bytes()).bind(branch).bind(xid).bind(branch).bind(xid).bind(branch).bind(xid).fetch_one(&mut admin).await.unwrap();
            assert_eq!(row, (1, 1, 1, 3, 1, 1));
            snapshots.push(serde_json::json!({"xid":xid,"branch_id":branch,"counts":[row.0,row.1,row.2,row.3,row.4,row.5]}));
        }
        let checkpoint = serde_json::json!({"round":round,"concurrency":concurrency,"identities":identities,"snapshots":snapshots,"observed_us":epoch.elapsed().as_micros() as u64});
        std::fs::write(
            out.join(format!("round-{round}.json")),
            serde_json::to_vec_pretty(&checkpoint).unwrap(),
        )
        .unwrap();
        release.wait().await;
        while let Some(result) = tasks.join_next().await {
            let (id, xid, branch, commit, started, committed, end_started, observed_status) =
                result.unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let row:(u8,u8,u8,u8)=sqlx::query_as("SELECT phase,terminal,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?").bind(xid.as_str().as_bytes()).bind(branch).fetch_one(&mut admin).await.unwrap();
                let remaining: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM global_table WHERE xid=?")
                        .bind(xid.as_str())
                        .fetch_one(&mut admin)
                        .await
                        .unwrap();
                if row == if commit { (1, 0, 1, 1) } else { (2, 1, 0, 0) } && remaining == 0 {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "original branch terminal/TC cleanup"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let identity =
                BranchIdentity::new(xid.as_str().into(), branch, key.clone(), None).unwrap();
            assert_eq!(
                store
                    .lease(
                        identity,
                        if commit {
                            BranchPhase::Commit
                        } else {
                            BranchPhase::Rollback
                        }
                    )
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Completed)
            );
            let actual: Vec<(i32, String)> =
                sqlx::query_as("SELECT id,v FROM undo_work WHERE id IN (?,?,?) ORDER BY id")
                    .bind(id)
                    .bind(id + 10000)
                    .bind(id + 20000)
                    .fetch_all(&mut admin)
                    .await
                    .unwrap();
            assert_eq!(
                actual,
                if commit {
                    vec![(id, "updated".into()), (id + 10000, "inserted".into())]
                } else {
                    vec![(id, "before".into()), (id + 20000, "delete_before".into())]
                }
            );
            results.push(serde_json::json!({"id":id,"xid":xid.as_str(),"branch_id":branch,"commit":commit,"started_us":started,"committed_us":committed,"end_started_us":end_started,"observed_end_status":format!("{observed_status:?}"),"finished_us":epoch.elapsed().as_micros() as u64}));
        }
    }
    assert_eq!(results.len(), concurrency * rounds);
    let remaining:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM global_table),(SELECT COUNT(*) FROM branch_table),(SELECT COUNT(*) FROM lock_table)").fetch_one(&mut admin).await.unwrap();
    assert_eq!(remaining, (0, 0, 0));
    rm.shutdown().await.unwrap();
    let sampler = tm.connection_sampler();
    Arc::try_unwrap(tm).ok().unwrap().shutdown().await.unwrap();
    let snapshot = sampler.snapshot();
    assert!(snapshot.closed);
    assert!(snapshot
        .slots
        .iter()
        .all(|s| s.connection.as_ref().is_none_or(|c| c.pending == 0)));
    ds.close().await.unwrap();
    results.sort_by_key(|v| v["id"].as_i64().unwrap());
    std::fs::write(out.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"AT_CONCURRENT_LOAD_VERIFIED","concurrency":concurrency,"rounds":rounds,"transactions":results,"wall_us":epoch.elapsed().as_micros() as u64,"tc_remaining":[0,0,0],"tm_closed":snapshot.closed})).unwrap()).unwrap();
}
