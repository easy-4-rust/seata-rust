use super::{observation_dir, owned_mysql, pool_at, tm_connection, undo_fixture};
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResource, FileBranchStore,
    JournalOptions, ResourceCatalog, ResourceManager,
};
use sqlx::MySqlConnection;
use std::{sync::Arc, time::Duration};

pub async fn reset() -> MySqlConnection {
    let mut admin = undo_fixture::reset().await;
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
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(source.resource_id().unwrap().into_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    source.close().await.unwrap();
    admin
}

pub struct JavaTmAt {
    source: MySqlDataSource,
    adapter: Arc<MySqlDataSourceManager>,
    journal: FileBranchStore,
    rm: ResourceManager,
}

impl JavaTmAt {
    pub async fn start(journal_name: &str) -> Self {
        let source = MySqlDataSource::connect(
            owned_mysql::options(None).with_java_tc_mysql_db_lock_profile(),
        )
        .await
        .unwrap();
        let adapter = Arc::new(
            MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default)
                .unwrap(),
        );
        let journal = FileBranchStore::open(
            observation_dir().join(journal_name),
            JournalOptions::new(16, 1_048_576, 65_536, 16).unwrap(),
        )
        .await
        .unwrap();
        let catalog = ResourceCatalog::new(4, 32_767).unwrap();
        catalog.register(adapter.clone()).unwrap();
        let (_, identity) = tm_connection();
        let rm = ResourceManager::builder(
            pool_at(std::env::var("SEATA_RM_ENDPOINT").unwrap().parse().unwrap()),
            identity,
            catalog,
            journal.clone(),
        )
        .request_timeout(Duration::from_secs(3))
        .callback_timeout(Duration::from_secs(5))
        .build()
        .unwrap();
        tokio::time::timeout(Duration::from_secs(8), async {
            while rm.connection_snapshot().ready() == 0 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        Self {
            source,
            adapter,
            journal,
            rm,
        }
    }

    pub async fn commit_phase_one(&self, xid: &str) {
        let mut local = self.source.begin().await.unwrap();
        local
            .execute(
                &SqlRecognizerFactory::recognize(
                    "INSERT INTO undo_work(id,v) VALUES(1,'tm-takeover')",
                )
                .unwrap(),
                &[],
            )
            .await
            .unwrap();
        let checked = self
            .adapter
            .prepare_transaction(local, &self.rm.client(), xid, None)
            .await
            .unwrap()
            .lock_commit_preconditions()
            .await
            .unwrap();
        checked
            .commit_changed_and_report(&self.adapter, &self.rm.client())
            .await
            .unwrap();
    }

    pub async fn branch_before_end(&self, xid: &str, admin: &mut MySqlConnection) -> i64 {
        let (branch, resource, started): (i64, Vec<u8>, u8) = sqlx::query_as(
            "SELECT branch_id,resource_id,report_started FROM seata_rs_at_commit_attempt WHERE xid=? AND attempt_state=1",
        ).bind(xid.as_bytes()).fetch_one(&mut *admin).await.unwrap();
        assert_eq!(resource, self.adapter.key().resource_id().as_bytes());
        assert_eq!(started, 1);
        let before: (i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM undo_work WHERE id=1 AND v='tm-takeover'), (SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0), (SELECT COUNT(*) FROM lock_table WHERE xid=? AND branch_id=? AND pk='1')",
        ).bind(xid).bind(branch).bind(xid).bind(branch).fetch_one(admin).await.unwrap();
        assert_eq!(before, (1, 1, 1));
        branch
    }

    pub async fn verify_terminal(
        &self,
        xid: &str,
        branch: i64,
        rollback: bool,
        admin: &mut MySqlConnection,
    ) {
        let phase = if rollback {
            BranchPhase::Rollback
        } else {
            BranchPhase::Commit
        };
        let expected = if rollback { (2, 1, 0) } else { (1, 0, 1) };
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let state: (u8, u8, u8) = sqlx::query_as(
                    "SELECT phase,terminal,commit_terminal FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?",
                ).bind(xid.as_bytes()).bind(branch).fetch_one(&mut *admin).await.unwrap();
                if state == expected {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        let branch_identity =
            BranchIdentity::new(xid.into(), branch, self.adapter.key().clone(), None).unwrap();
        assert_eq!(
            self.journal
                .lease(branch_identity, phase)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        // 托管回滚删除 Normal Undo，并保留同一原身份的 GlobalFinished 围栏。
        let undo: Vec<(String, i64, i32, String, Vec<u8>)> =
            sqlx::query_as("SELECT xid,branch_id,log_status,context,rollback_info FROM undo_log")
                .fetch_all(&mut *admin)
                .await
                .unwrap();
        if rollback {
            assert_eq!(
                undo,
                vec![(
                    xid.to_owned(),
                    branch,
                    1,
                    "serializer=jackson&compressorType=NONE".into(),
                    b"{}".to_vec(),
                )]
            );
            let late_undo = sqlx::query(
                "INSERT INTO undo_log (xid,branch_id,log_status,context,rollback_info,log_created,log_modified) VALUES (?,?,0,'serializer=jackson&compressorType=NONE',X'7B7D',UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))",
            ).bind(xid).bind(branch).execute(&mut *admin).await.unwrap_err();
            let duplicate = late_undo
                .as_database_error()
                .and_then(|error| error.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
                .map(sqlx::mysql::MySqlDatabaseError::number);
            assert_eq!(duplicate, Some(1062));
        } else {
            assert!(undo.is_empty());
        }
        assert_eq!(
            undo_fixture::count(admin, "undo_work").await,
            i64::from(!rollback)
        );
    }

    pub async fn shutdown(self) {
        self.rm.shutdown().await.unwrap();
        self.source.close().await.unwrap();
    }
}
