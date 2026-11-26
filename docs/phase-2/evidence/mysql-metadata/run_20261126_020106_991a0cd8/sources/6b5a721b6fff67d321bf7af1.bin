//! 真实 TC TCP 对端与 MySQL 008 来源表的联合门禁。
#![allow(clippy::unwrap_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "../../seata_tm/tests/support/tm_peer.rs"]
mod tm_fixture;

use seata_db_mysql::{DbError, MySqlDataSource, MySqlLauncherStore, MySqlOptions};
use seata_protocol::{GlobalStatus, Message};
use seata_tm::{
    DurableTmError, LauncherOrigin, LauncherProcessMonitor, LauncherStore, TmError, TmOperation,
    Xid,
};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Row};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

async fn apply_launcher_migrations(admin: &mut sqlx::MySqlConnection) {
    for migration in [
        include_str!("../migrations/008_seata_rs_tm_launcher_v1.sql"),
        include_str!("../migrations/009_seata_rs_tm_launcher_owner_instance.sql"),
        include_str!("../migrations/010_seata_rs_tm_launcher_stop_scope.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut *admin).await.unwrap();
    }
}

async fn launcher_account(
    admin: &mut sqlx::MySqlConnection,
    account: &str,
    role: &str,
) -> (MySqlOptions, sqlx::MySqlConnection) {
    // 夹具中账号和角色为固定值，密码由 MySQL 随机生成。
    let row = sqlx::query(&format!(
        "CREATE USER '{account}'@'%' IDENTIFIED BY RANDOM PASSWORD"
    ))
    .fetch_one(&mut *admin)
    .await
    .unwrap();
    let password: String = row.get(2);
    sqlx::raw_sql(&format!(
        "GRANT '{role}' TO '{account}'@'%'; SET DEFAULT ROLE '{role}' TO '{account}'@'%'"
    ))
    .execute(&mut *admin)
    .await
    .unwrap();
    let port = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let database = std::env::var("SEATA_MYSQL_DATABASE").unwrap();
    let connection = sqlx::MySqlConnection::connect_with(
        &sqlx::mysql::MySqlConnectOptions::new()
            .host("127.0.0.1")
            .port(port)
            .username(account)
            .password(&password)
            .database(&database)
            .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled),
    )
    .await
    .unwrap();
    let options = MySqlOptions::new("127.0.0.1".into(), port, account.into(), password, database)
        .unwrap()
        .with_insecure_loopback_tls()
        .unwrap();
    (options, connection)
}

async fn assert_launcher_permission_denied(connection: &mut sqlx::MySqlConnection, sql: &str) {
    let error = sqlx::query(sql).execute(connection).await.unwrap_err();
    let number = error
        .as_database_error()
        .and_then(|error| error.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
        .map(sqlx::mysql::MySqlDatabaseError::number);
    assert!(matches!(number, Some(1142 | 1143)), "{sql}: {error}");
}

#[tokio::test]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn existing_launcher_accounts_gain_only_stop_scope_grants() {
    let mut admin = owned_mysql::admin().await;
    for migration in [
        include_str!("../migrations/008_seata_rs_tm_launcher_v1.sql"),
        include_str!("../migrations/009_seata_rs_tm_launcher_owner_instance.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }

    // 复原 009 的列级授权，再用既有账号和默认角色执行增量升级。
    sqlx::raw_sql(
        "CREATE ROLE 'seata_tm_launcher_reader', 'seata_tm_launcher_writer', 'seata_tm_launcher_recovery';
         GRANT SELECT ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_reader';
         GRANT SELECT,
           INSERT (schema_version, xid_digest, xid, tc_endpoint, application_id, service_group,
                   begin_exception_code, begin_extra_data, business_intent, owner_instance_id,
                   deadline_at, created_at, modified_at),
           UPDATE (end_operation, send_started, modified_at)
         ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_writer';
         GRANT SELECT, UPDATE (owner_instance_id, owner_epoch, modified_at)
         ON seata_rs_tm_launcher_v1 TO 'seata_tm_launcher_recovery';",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    let (_, mut writer) =
        launcher_account(&mut admin, "tm_runtime", "seata_tm_launcher_writer").await;
    let (_, mut recovery) =
        launcher_account(&mut admin, "tm_controller", "seata_tm_launcher_recovery").await;
    let (_, mut reader) =
        launcher_account(&mut admin, "tm_observer", "seata_tm_launcher_reader").await;
    sqlx::query(
        "INSERT INTO seata_rs_tm_launcher_v1
           (schema_version, xid_digest, xid, tc_endpoint, application_id, service_group,
            begin_exception_code, business_intent, owner_instance_id, deadline_at, created_at, modified_at)
         VALUES (1, UNHEX(SHA2('tc:legacy-role', 256)), 'tc:legacy-role', '127.0.0.1:8091',
                 'tm-app', 'tm-group', 0, 'order-legacy', REPEAT('a', 32),
                 UTC_TIMESTAMP(6) + INTERVAL 60 SECOND, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))",
    )
    .execute(&mut writer)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/010_seata_rs_tm_launcher_stop_scope.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();

    let insert = "INSERT INTO seata_rs_tm_launcher_v1
        (schema_version, xid_digest, xid, tc_endpoint, application_id, service_group,
         begin_exception_code, business_intent, owner_instance_id, owner_stop_scope,
         deadline_at, created_at, modified_at)
        VALUES (1, UNHEX(SHA2('tc:scoped-role', 256)), 'tc:scoped-role', '127.0.0.1:8091',
                'tm-app', 'tm-group', 0, 'order-scoped', REPEAT('a', 32), REPEAT('b', 32),
                UTC_TIMESTAMP(6) + INTERVAL 60 SECOND, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))";
    let transfer = "UPDATE seata_rs_tm_launcher_v1
        SET owner_instance_id=REPEAT('c', 32), owner_epoch=2,
            owner_stop_scope=REPEAT('d', 32), modified_at=UTC_TIMESTAMP(6)
        WHERE xid='tc:scoped-role' AND owner_epoch=1 AND owner_stop_scope=REPEAT('b', 32)";
    assert_launcher_permission_denied(&mut writer, insert).await;
    assert_launcher_permission_denied(&mut recovery, transfer).await;
    for _ in 0..2 {
        sqlx::raw_sql(include_str!(
            "../sql/tm_launcher_stop_scope_upgrade_grants.sql"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
    }

    // 重复授权后，原连接可使用新列，且职责仍由数据库分开约束。
    assert_launcher_permission_denied(&mut reader, insert).await;
    assert_launcher_permission_denied(&mut reader, transfer).await;
    assert_launcher_permission_denied(&mut recovery, insert).await;
    assert_launcher_permission_denied(&mut writer, transfer).await;
    assert_launcher_permission_denied(
        &mut writer,
        "UPDATE seata_rs_tm_launcher_v1 SET owner_stop_scope=NULL",
    )
    .await;
    assert_launcher_permission_denied(
        &mut recovery,
        "UPDATE seata_rs_tm_launcher_v1 SET send_started=1",
    )
    .await;
    assert_eq!(
        sqlx::query(insert)
            .execute(&mut writer)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_eq!(
        sqlx::query(transfer)
            .execute(&mut recovery)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    let rows: Vec<(u64, Vec<u8>, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT owner_epoch, owner_instance_id, owner_stop_scope
         FROM seata_rs_tm_launcher_v1 ORDER BY xid",
    )
    .fetch_all(&mut reader)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (1, vec![b'a'; 32], None),
            (2, vec![b'c'; 32], Some(vec![b'd'; 32]))
        ]
    );
}

struct ExitAfterMarkerStore(MySqlLauncherStore, bool);
struct LoseMarkerAckStore(MySqlLauncherStore);

impl LauncherStore for LoseMarkerAckStore {
    async fn record_begin(
        &self,
        origin: &LauncherOrigin,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.record_begin(origin).await
    }

    async fn record_end_intent(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.record_end_intent(origin, operation).await
    }

    async fn mark_send_started(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.mark_send_started(origin, operation).await?;
        Err("persisted marker acknowledgement lost".into())
    }
}

impl LauncherStore for ExitAfterMarkerStore {
    async fn record_begin(
        &self,
        origin: &LauncherOrigin,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.record_begin(origin).await
    }

    async fn record_end_intent(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.record_end_intent(origin, operation).await?;
        if self.1 {
            std::process::exit(0);
        }
        Ok(())
    }

    async fn mark_send_started(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.mark_send_started(origin, operation).await?;
        // 故障注入：确认门闩已经提交，尚未返回到 TM 的网络发送路径。
        std::process::exit(0);
    }
}

#[tokio::test]
#[ignore = "child process controlled by marker_exit_retains_one_way_send_claim"]
async fn tm_launcher_exit_after_marker_child() {
    let endpoint = std::env::var("SEATA_TM_FIXTURE_ENDPOINT")
        .unwrap()
        .parse()
        .unwrap();
    let data_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = ExitAfterMarkerStore(
        MySqlLauncherStore::new(data_source),
        std::env::var_os("SEATA_TM_FIXTURE_EXIT_AFTER_INTENT").is_some(),
    );
    let tm = seata_tm::TransactionManager::builder(
        tm_fixture::settings(endpoint),
        tm_fixture::identity(),
    )
    .request_timeout(Duration::from_secs(2))
    .launcher_stop_scope([9; 32])
    .build()
    .unwrap();
    tm.announce_supervised_instance(std::io::stdout().lock())
        .unwrap();
    let tx = tm
        .begin_durable(&tm_fixture::config(), "order-crash", &store)
        .await
        .unwrap();
    if std::env::var_os("SEATA_TM_FIXTURE_RECOVERY_ROLLBACK").is_some() {
        let _ = tx.rollback().await;
    } else {
        let _ = tx.commit().await;
    }
    panic!("fault injection should have exited immediately after marker commit");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn marker_exit_retains_one_way_send_claim() {
    supervised_exit(false, false).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn supervised_exit_allows_only_one_owner_transfer() {
    supervised_exit(true, false).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn transferred_rollback_lost_reply_stays_unknown() {
    supervised_exit(true, true).await;
}

async fn supervised_exit(after_intent: bool, rollback: bool) {
    let operation = if rollback {
        TmOperation::Rollback
    } else {
        TmOperation::Commit
    };
    let code = if rollback { 2 } else { 1 };
    let mut admin = owned_mysql::admin().await;
    apply_launcher_migrations(&mut admin).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (begun, begin_wait) = tokio::sync::oneshot::channel();
    let (release, release_wait) = tokio::sync::oneshot::channel();
    let before_rotation = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let after_rotation = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let peer_before_rotation = before_rotation.clone();
    let peer_after_rotation = after_rotation.clone();
    let server = tokio::spawn(async move {
        let mut socket = tm_fixture::registered(&listener).await;
        // 已收到报告之后才可能出现 Begin，暂扣响应以保证检查时子进程仍活着。
        let begin = tm_fixture::peer::read(&mut socket).await;
        assert!(matches!(begin.body, Some(Message::GlobalBeginRequest(_))));
        begun.send(()).unwrap();
        release_wait.await.unwrap();
        tm_fixture::peer::write(
            &mut socket,
            tm_fixture::peer::reply(
                begin.id,
                Message::GlobalBeginResponse(seata_protocol::transaction::GlobalBeginResponse {
                    response: tm_fixture::peer::success(),
                    xid: Some("tc:marker-exit".into()),
                    extra_data: None,
                }),
            ),
        )
        .await;
        let mut bytes = [0_u8; 16];
        let read = tokio::time::timeout(Duration::from_secs(10), socket.read(&mut bytes))
            .await
            .unwrap();
        assert!(
            matches!(&read, Ok(0))
                || matches!(&read, Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset),
            "child must exit before any end frame: {read:?}"
        );
        if after_intent {
            let mut recovery = tm_fixture::registered(&listener).await;
            for (index, status) in [
                GlobalStatus::Finished,
                GlobalStatus::Begin,
                GlobalStatus::Begin,
                GlobalStatus::Begin,
                GlobalStatus::Begin,
                GlobalStatus::Begin,
                GlobalStatus::Begin,
            ]
            .into_iter()
            .enumerate()
            {
                let query = tm_fixture::peer::read(&mut recovery).await;
                let Some(Message::GlobalStatusRequest(request)) = query.body else {
                    panic!("only a status query is allowed during owner transfer");
                };
                assert_eq!(request.xid.as_deref(), Some("tc:marker-exit"));
                if index == 1 {
                    let mut witness = owned_mysql::admin().await;
                    sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET created_at=DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 2 SECOND), deadline_at=DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 1 SECOND)")
                        .execute(&mut witness).await.unwrap();
                }
                if index == 3 {
                    peer_before_rotation.wait().await;
                    peer_after_rotation.wait().await;
                }
                tm_fixture::peer::write(
                    &mut recovery,
                    tm_fixture::peer::reply(
                        query.id,
                        Message::GlobalStatusResponse(
                            seata_protocol::transaction::GlobalStatusResponse {
                                response: tm_fixture::peer::success(),
                                global_status: status,
                            },
                        ),
                    ),
                )
                .await;
            }
            let end = tm_fixture::peer::read(&mut recovery).await;
            match end.body {
                Some(Message::GlobalCommitRequest(request)) if !rollback => {
                    assert_eq!(request.xid.as_deref(), Some("tc:marker-exit"));
                }
                Some(Message::GlobalRollbackRequest(request)) if rollback => {
                    assert_eq!(request.xid.as_deref(), Some("tc:marker-exit"));
                }
                _ => panic!("expected the original end operation after the send marker"),
            }
            let mut witness = owned_mysql::admin().await;
            let state: (u64, u8, u8) = sqlx::query_as(
                "SELECT owner_epoch,end_operation,send_started FROM seata_rs_tm_launcher_v1",
            )
            .fetch_one(&mut witness)
            .await
            .unwrap();
            assert_eq!(state, (2, code, 1));
            if rollback {
                // 结束帧已经收到，断开连接以模拟原 TC 回包丢失。
                return;
            }
            tm_fixture::peer::write(
                &mut recovery,
                tm_fixture::peer::reply(
                    end.id,
                    tm_fixture::end_response(
                        true,
                        GlobalStatus::Committed,
                        tm_fixture::peer::success(),
                    ),
                ),
            )
            .await;
            tm_fixture::peer::eof(&mut recovery).await;
        }
    });
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--ignored",
            "--exact",
            "tm_launcher_exit_after_marker_child",
        ])
        .env("SEATA_TM_FIXTURE_ENDPOINT", endpoint.to_string());
    if after_intent {
        sqlx::raw_sql(include_str!("../sql/tm_launcher_roles.sql"))
            .execute(&mut admin)
            .await
            .unwrap();
        command.env("SEATA_TM_FIXTURE_EXIT_AFTER_INTENT", "1");
    }
    if rollback {
        command.env("SEATA_TM_FIXTURE_RECOVERY_ROLLBACK", "1");
    }
    let mut monitor = LauncherProcessMonitor::spawn(&mut command).unwrap();
    tokio::time::timeout(Duration::from_secs(5), begin_wait)
        .await
        .unwrap()
        .unwrap();
    assert!(monitor.try_confirm_exit().unwrap().is_none());
    release.send(()).unwrap();
    let stopped = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(proof) = monitor.try_confirm_exit().unwrap() {
                break proof;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let state: (Vec<u8>, u8, u8, u64, Vec<u8>) = sqlx::query_as(
        "SELECT xid,end_operation,send_started,owner_epoch,owner_instance_id FROM seata_rs_tm_launcher_v1",
    )
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(state.0, b"tc:marker-exit");
    assert_eq!(
        (state.1, state.2, state.3),
        (code, u8::from(!after_intent), 1)
    );
    let persisted: [u8; 32] = state.4.try_into().unwrap();
    assert!(monitor.verifies(&stopped, &persisted));
    let mut unrelated = persisted;
    unrelated[0] ^= 1;
    assert!(!monitor.verifies(&stopped, &unrelated));
    let mut other_command = std::process::Command::new(std::env::current_exe().unwrap());
    other_command.arg("--list");
    let mut other_monitor = LauncherProcessMonitor::spawn(&mut other_command).unwrap();
    assert!(!other_monitor.verifies(&stopped, &persisted));
    let missing = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match other_monitor.try_confirm_exit() {
                Ok(None) => tokio::time::sleep(Duration::from_millis(10)).await,
                result => break result,
            }
        }
    })
    .await
    .unwrap();
    assert!(matches!(missing, Err(error) if error.kind() == std::io::ErrorKind::InvalidData));
    if after_intent {
        sqlx::raw_sql(include_str!("../sql/tm_launcher_recovery_role.sql"))
            .execute(&mut admin)
            .await
            .unwrap();
        let (options, mut account) =
            launcher_account(&mut admin, "tm_recovery", "seata_tm_launcher_recovery").await;
        assert_launcher_permission_denied(
            &mut account,
            "UPDATE seata_rs_tm_launcher_v1 SET send_started=1",
        )
        .await;
        let source = MySqlDataSource::connect(options).await.unwrap();
        let store = MySqlLauncherStore::new(source.clone());
        let xid = Xid::parse("tc:marker-exit".into()).unwrap();
        let snapshot = store.load_snapshot(&xid).await.unwrap().unwrap();
        assert_eq!(snapshot.owner_stop_scope(), Some(&[9; 32]));
        let (tm, authority) = seata_tm::TransactionManager::builder(
            tm_fixture::settings(endpoint),
            tm_fixture::identity(),
        )
        .request_timeout(Duration::from_secs(2))
        .launcher_stop_scope([10; 32])
        .build_with_recovered_end_authority()
        .unwrap();
        assert!(store
            .transfer_stopped_owner(&snapshot, &other_monitor, &stopped, &tm, "order-crash")
            .await
            .is_err());
        let rejected = store
            .transfer_stopped_owner(&snapshot, &monitor, &stopped, &tm, "order-crash")
            .await;
        assert!(
            matches!(rejected, Err(seata_db_mysql::MySqlLauncherClaimError::Observed(status)) if status == GlobalStatus::Finished)
        );
        let expired = store
            .transfer_stopped_owner(&snapshot, &monitor, &stopped, &tm, "order-crash")
            .await;
        assert!(matches!(
            expired,
            Err(seata_db_mysql::MySqlLauncherClaimError::Store(
                DbError::RecoveryConflict
            ))
        ));
        let unchanged = store.load_snapshot(&xid).await.unwrap().unwrap();
        assert!(unchanged.deadline_passed());
        assert_eq!(unchanged.owner_epoch(), 1);
        // 夹具恢复期限，独立验证两个并发申请对同一代际的竞争。
        sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET deadline_at=DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 20 SECOND)")
            .execute(&mut admin).await.unwrap();
        let key = ring::signature::Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())
            .unwrap();
        let signer = seata_tm::LauncherStopSigner::from_pkcs8(key.as_ref(), [9; 32]).unwrap();
        let (verifier, mut rotation) =
            seata_tm::LauncherStopVerifier::new_rotatable(signer.public_key(), [9; 32]);
        let challenge = verifier
            .challenge(
                snapshot.owner_instance_id(),
                tm.owner_instance_id(),
                Duration::from_secs(10),
            )
            .unwrap();
        assert!(other_monitor
            .sign_exit_proof(&stopped, &signer, challenge.nonce())
            .is_none());
        let (mut requester, mut responder) = tokio::io::duplex(128);
        let (signed, served) = tokio::join!(
            seata_tm::LauncherStopExchange::request(
                &mut requester,
                &verifier,
                &challenge,
                Duration::from_secs(2)
            ),
            seata_tm::LauncherStopExchange::respond(
                &mut responder,
                &monitor,
                &stopped,
                &signer,
                Duration::from_secs(2)
            ),
        );
        served.unwrap();
        let signed = signed.unwrap();
        sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET owner_stop_scope=NULL WHERE xid=?")
            .bind(xid.as_str().as_bytes())
            .execute(&mut admin)
            .await
            .unwrap();
        let unconfigured = store.load_snapshot(&xid).await.unwrap().unwrap();
        assert_eq!(unconfigured.owner_stop_scope(), None);
        assert!(matches!(
            store
                .transfer_attested_owner(
                    &unconfigured,
                    &verifier,
                    &challenge,
                    &signed,
                    &tm,
                    "order-crash"
                )
                .await,
            Err(seata_db_mysql::MySqlLauncherClaimError::Store(
                DbError::RecoveryConflict
            ))
        ));
        assert!(matches!(
            store
                .transfer_attested_owner(
                    &snapshot,
                    &verifier,
                    &challenge,
                    &signed,
                    &tm,
                    "order-crash"
                )
                .await,
            Err(seata_db_mysql::MySqlLauncherClaimError::Store(
                DbError::RecoveryConflict
            ))
        ));
        assert_eq!(
            store
                .load_snapshot(&xid)
                .await
                .unwrap()
                .unwrap()
                .owner_epoch(),
            1
        );
        sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET owner_stop_scope=? WHERE xid=?")
            .bind([9_u8; 32].as_slice())
            .bind(xid.as_str().as_bytes())
            .execute(&mut admin)
            .await
            .unwrap();
        let mut changed = signed;
        changed[0] ^= 1;
        assert!(store
            .transfer_attested_owner(
                &snapshot,
                &verifier,
                &challenge,
                &changed,
                &tm,
                "order-crash"
            )
            .await
            .is_err());
        let other_scope = seata_tm::LauncherStopVerifier::new(signer.public_key(), [8; 32]);
        let wrong_scope = other_scope
            .challenge(
                snapshot.owner_instance_id(),
                tm.owner_instance_id(),
                Duration::from_secs(10),
            )
            .unwrap();
        let same_nonce_other_scope = monitor
            .sign_exit_proof(&stopped, &signer, wrong_scope.nonce())
            .unwrap();
        assert!(!other_scope.verifies(
            &wrong_scope,
            &same_nonce_other_scope,
            snapshot.owner_instance_id(),
            tm.owner_instance_id()
        ));
        let wrong_host_signer =
            seata_tm::LauncherStopSigner::from_pkcs8(key.as_ref(), [8; 32]).unwrap();
        let wrong_host_proof = monitor
            .sign_exit_proof(&stopped, &wrong_host_signer, wrong_scope.nonce())
            .unwrap();
        assert!(other_scope.verifies(
            &wrong_scope,
            &wrong_host_proof,
            snapshot.owner_instance_id(),
            tm.owner_instance_id()
        ));
        assert!(matches!(
            store
                .transfer_attested_owner(
                    &snapshot,
                    &other_scope,
                    &wrong_scope,
                    &wrong_host_proof,
                    &tm,
                    "order-crash"
                )
                .await,
            Err(seata_db_mysql::MySqlLauncherClaimError::Store(
                DbError::RecoveryConflict
            ))
        ));
        let foreign_key =
            ring::signature::Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())
                .unwrap();
        let foreign_signer =
            seata_tm::LauncherStopSigner::from_pkcs8(foreign_key.as_ref(), [9; 32]).unwrap();
        let foreign_signature = monitor
            .sign_exit_proof(&stopped, &foreign_signer, challenge.nonce())
            .unwrap();
        assert!(!verifier.verifies(
            &challenge,
            &foreign_signature,
            snapshot.owner_instance_id(),
            tm.owner_instance_id()
        ));
        assert!(!verifier.verifies(
            &challenge,
            &signed,
            tm.owner_instance_id(),
            snapshot.owner_instance_id()
        ));
        let other_challenge = verifier
            .challenge(
                snapshot.owner_instance_id(),
                tm.owner_instance_id(),
                Duration::from_secs(10),
            )
            .unwrap();
        assert!(!verifier.verifies(
            &other_challenge,
            &signed,
            snapshot.owner_instance_id(),
            tm.owner_instance_id()
        ));
        let expiring = verifier
            .challenge(
                snapshot.owner_instance_id(),
                tm.owner_instance_id(),
                Duration::from_millis(1),
            )
            .unwrap();
        let expired_signature = monitor
            .sign_exit_proof(&stopped, &signer, expiring.nonce())
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!verifier.verifies(
            &expiring,
            &expired_signature,
            snapshot.owner_instance_id(),
            tm.owner_instance_id()
        ));
        rotation
            .replace(&[signer.public_key(), foreign_signer.public_key()])
            .unwrap();
        assert!(verifier.verifies(
            &challenge,
            &signed,
            snapshot.owner_instance_id(),
            tm.owner_instance_id()
        ));
        assert!(verifier.verifies(
            &challenge,
            &foreign_signature,
            snapshot.owner_instance_id(),
            tm.owner_instance_id()
        ));
        let (in_flight, ()) = tokio::join!(
            store.transfer_attested_owner(
                &snapshot,
                &verifier,
                &challenge,
                &signed,
                &tm,
                "order-crash"
            ),
            async {
                tokio::time::timeout(Duration::from_secs(3), before_rotation.wait())
                    .await
                    .unwrap();
                rotation.replace(&[foreign_signer.public_key()]).unwrap();
                after_rotation.wait().await;
            }
        );
        assert!(matches!(
            in_flight,
            Err(seata_db_mysql::MySqlLauncherClaimError::Store(
                DbError::RecoveryConflict
            ))
        ));
        assert_eq!(
            store
                .load_snapshot(&xid)
                .await
                .unwrap()
                .unwrap()
                .owner_epoch(),
            1
        );
        assert!(matches!(
            store
                .transfer_attested_owner(
                    &snapshot,
                    &verifier,
                    &challenge,
                    &signed,
                    &tm,
                    "order-crash"
                )
                .await,
            Err(seata_db_mysql::MySqlLauncherClaimError::Store(
                DbError::RecoveryConflict
            ))
        ));
        let (first, second) = tokio::join!(
            store.transfer_attested_owner(
                &snapshot,
                &verifier,
                &challenge,
                &foreign_signature,
                &tm,
                "order-crash"
            ),
            store.transfer_attested_owner(
                &snapshot,
                &verifier,
                &challenge,
                &foreign_signature,
                &tm,
                "order-crash"
            ),
        );
        let claim = match (first, second) {
            (
                Ok(claim),
                Err(seata_db_mysql::MySqlLauncherClaimError::Store(DbError::RecoveryConflict)),
            )
            | (
                Err(seata_db_mysql::MySqlLauncherClaimError::Store(DbError::RecoveryConflict)),
                Ok(claim),
            ) => claim,
            _ => panic!("expected one confirmed transfer and one CAS conflict"),
        };
        assert_eq!(claim.epoch(), 2);
        let updated = store.load_snapshot(&xid).await.unwrap().unwrap();
        assert_eq!(updated.owner_epoch(), 2);
        assert_eq!(updated.owner_instance_id(), tm.owner_instance_id());
        assert_eq!(updated.owner_stop_scope(), Some(&[10; 32]));
        assert_eq!(updated.end_operation(), Some(operation));
        assert!(!updated.send_started());
        let (writer_options, _writer_account) =
            launcher_account(&mut admin, "tm_runtime", "seata_tm_launcher_writer").await;
        let writer = MySqlDataSource::connect(writer_options).await.unwrap();
        let write_store = MySqlLauncherStore::new(writer.clone());
        let outcome = claim.finish(&authority, &write_store).await;
        if rollback {
            let Err(DurableTmError::Tm(error)) = outcome else {
                panic!("a lost recovered Rollback reply must remain unknown");
            };
            assert!(error.is_unknown_outcome());
            assert!(tm.recover_unknown(&error).is_err());
        } else {
            assert_eq!(outcome.unwrap().status(), GlobalStatus::Committed);
        }
        let sent = store.load_snapshot(&xid).await.unwrap().unwrap();
        assert!(sent.send_started());
        assert!(matches!(
            store
                .transfer_stopped_owner(&sent, &monitor, &stopped, &tm, "order-crash")
                .await,
            Err(seata_db_mysql::MySqlLauncherClaimError::Store(
                DbError::RecoveryConflict
            ))
        ));
        tm.shutdown().await.unwrap();
        writer.close().await.unwrap();
        source.close().await.unwrap();
    }
    server.await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn marker_ack_loss_after_persisted_cas_never_emits_end_frame() {
    let mut admin = owned_mysql::admin().await;
    apply_launcher_migrations(&mut admin).await;
    let data_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = LoseMarkerAckStore(MySqlLauncherStore::new(data_source.clone()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = tm_fixture::manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = tm_fixture::registered(&listener).await;
        tm_fixture::begin(&mut socket, Some("tc:marker-ack-lost")).await;
        let end = tokio::time::timeout(
            Duration::from_millis(250),
            tm_fixture::peer::read(&mut socket),
        )
        .await;
        assert!(
            end.is_err(),
            "no end frame may follow an unconfirmed marker"
        );
    });
    let tx = tm
        .begin_durable(&tm_fixture::config(), "order-marker-ack", &store)
        .await
        .unwrap();
    let error = tx.commit().await.unwrap_err();
    assert!(matches!(
        error,
        DurableTmError::Persistence {
            stage: "send marker",
            ..
        }
    ));
    let state: (u8, u8) = sqlx::query_as(
        "SELECT end_operation,send_started FROM seata_rs_tm_launcher_v1 WHERE xid=?",
    )
    .bind(b"tc:marker-ack-lost".as_slice())
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(state, (1, 1));
    server.await.unwrap();
    tm.shutdown().await.unwrap();
    data_source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn changed_owner_instance_blocks_original_end_cas() {
    let mut admin = owned_mysql::admin().await;
    apply_launcher_migrations(&mut admin).await;
    let data_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(data_source.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = tm_fixture::manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = tm_fixture::registered(&listener).await;
        tm_fixture::begin(&mut socket, Some("tc:owner-changed")).await;
        let end = tokio::time::timeout(
            Duration::from_millis(250),
            tm_fixture::peer::read(&mut socket),
        )
        .await;
        assert!(end.is_err(), "stale owner must not send an end frame");
    });
    let tx = tm
        .begin_durable(&tm_fixture::config(), "order-owner-changed", &store)
        .await
        .unwrap();
    let changed = [2_u8; 32];
    assert_ne!(tm.owner_instance_id(), &changed);
    sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET owner_instance_id=? WHERE xid=?")
        .bind(changed.as_slice())
        .bind(tx.xid().as_str().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let error = tx.commit().await.unwrap_err();
    assert!(matches!(
        error,
        DurableTmError::Persistence {
            stage: "end intent",
            ..
        }
    ));
    let state: (u8, u8) = sqlx::query_as(
        "SELECT end_operation,send_started FROM seata_rs_tm_launcher_v1 WHERE xid=?",
    )
    .bind(b"tc:owner-changed".as_slice())
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(state, (0, 0));
    server.await.unwrap();
    tm.shutdown().await.unwrap();
    data_source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn commit_response_loss_keeps_send_claim_and_denies_memory_recovery() {
    let mut admin = owned_mysql::admin().await;
    apply_launcher_migrations(&mut admin).await;
    let data_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(data_source.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = tm_fixture::manager(listener.local_addr().unwrap(), Duration::from_millis(120));
    let server = tokio::spawn(async move {
        let mut socket = tm_fixture::registered(&listener).await;
        tm_fixture::begin(&mut socket, Some("tc:lost-end-reply")).await;
        let request = tm_fixture::peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(Message::GlobalCommitRequest(_))
        ));
        let mut witness = owned_mysql::admin().await;
        let state: (u8, u8) = sqlx::query_as(
            "SELECT end_operation,send_started FROM seata_rs_tm_launcher_v1 WHERE xid=?",
        )
        .bind(b"tc:lost-end-reply".as_slice())
        .fetch_one(&mut witness)
        .await
        .unwrap();
        assert_eq!(state, (1, 1));
        // 故障注入：收到了真实 Commit 帧，但丢弃响应并断开连接。
        drop(socket);
    });
    let tx = tm
        .begin_durable(&tm_fixture::config(), "order-lost-reply", &store)
        .await
        .unwrap();
    let error = tx.commit().await.unwrap_err();
    let DurableTmError::Tm(error @ TmError::UnknownOutcome { .. }) = error else {
        panic!("response loss must remain an unknown outcome");
    };
    assert!(tm.recover_unknown(&error).is_err());
    server.await.unwrap();
    let state: (u8, u8) = sqlx::query_as(
        "SELECT end_operation,send_started FROM seata_rs_tm_launcher_v1 WHERE xid=?",
    )
    .bind(b"tc:lost-end-reply".as_slice())
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(state, (1, 1));
    tm.shutdown().await.unwrap();
    data_source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn begin_source_and_end_markers_precede_real_commit_frame() {
    let mut admin = owned_mysql::admin().await;
    apply_launcher_migrations(&mut admin).await;
    sqlx::raw_sql(include_str!("../sql/tm_launcher_roles.sql"))
        .execute(&mut admin)
        .await
        .unwrap();
    let (write_options, mut writer) =
        launcher_account(&mut admin, "tm_runtime", "seata_tm_launcher_writer").await;
    let (read_options, mut reader) =
        launcher_account(&mut admin, "tm_observer", "seata_tm_launcher_reader").await;
    let data_source = MySqlDataSource::connect(write_options).await.unwrap();
    let read_source = MySqlDataSource::connect(read_options).await.unwrap();
    let inspect = MySqlLauncherStore::new(read_source.clone());
    let store = MySqlLauncherStore::new(data_source.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let tm = tm_fixture::manager(endpoint, Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = tm_fixture::registered(&listener).await;
        tm_fixture::begin(&mut socket, Some("tc:durable-real")).await;
        let request = tm_fixture::peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(Message::GlobalCommitRequest(_))
        ));
        // 独立观察者在收到二阶段帧时，发送标记已经是持久可见状态。
        let mut witness = owned_mysql::admin().await;
        let state: (u8, u8) = sqlx::query_as(
            "SELECT end_operation,send_started FROM seata_rs_tm_launcher_v1 WHERE xid=?",
        )
        .bind(b"tc:durable-real".as_slice())
        .fetch_one(&mut witness)
        .await
        .unwrap();
        assert_eq!(state, (1, 1));
        tm_fixture::peer::write(
            &mut socket,
            tm_fixture::peer::reply(
                request.id,
                tm_fixture::end_response(
                    true,
                    GlobalStatus::Committed,
                    tm_fixture::peer::success(),
                ),
            ),
        )
        .await;
        tm_fixture::peer::eof(&mut socket).await;
        let mut observer = tm_fixture::registered(&listener).await;
        let query = tm_fixture::peer::read(&mut observer).await;
        let Some(Message::GlobalStatusRequest(request)) = query.body else {
            panic!("expected read-only GlobalStatusRequest");
        };
        assert_eq!(request.xid.as_deref(), Some("tc:durable-real"));
        tm_fixture::peer::write(
            &mut observer,
            tm_fixture::peer::reply(
                query.id,
                Message::GlobalStatusResponse(seata_protocol::transaction::GlobalStatusResponse {
                    response: tm_fixture::peer::success(),
                    global_status: GlobalStatus::Finished,
                }),
            ),
        )
        .await;
        tm_fixture::peer::eof(&mut observer).await;
    });
    let tx = tm
        .begin_durable(&tm_fixture::config(), "order-123", &store)
        .await
        .unwrap();
    let snapshot = inspect.load_snapshot(tx.xid()).await.unwrap().unwrap();
    assert_eq!(snapshot.xid(), tx.xid());
    assert_eq!(snapshot.tc_endpoint(), endpoint);
    assert_eq!(snapshot.application_id(), "tm-app");
    assert_eq!(snapshot.service_group(), "tm-group");
    assert_eq!(snapshot.business_intent(), "order-123");
    assert_eq!(snapshot.owner_instance_id(), tm.owner_instance_id());
    assert_eq!(snapshot.end_operation(), None);
    assert!(!snapshot.send_started());
    let row = sqlx::query("SELECT xid,tc_endpoint,application_id,service_group,begin_extra_data,business_intent,end_operation,send_started,deadline_at>created_at AS valid_deadline FROM seata_rs_tm_launcher_v1")
        .fetch_one(&mut admin).await.unwrap();
    assert_eq!(row.get::<Vec<u8>, _>("xid"), b"tc:durable-real");
    assert_eq!(
        row.get::<Vec<u8>, _>("tc_endpoint"),
        endpoint.to_string().as_bytes()
    );
    assert_eq!(row.get::<Vec<u8>, _>("application_id"), b"tm-app");
    assert_eq!(row.get::<Vec<u8>, _>("service_group"), b"tm-group");
    assert_eq!(row.get::<Vec<u8>, _>("begin_extra_data"), b"server-extra");
    assert_eq!(row.get::<Vec<u8>, _>("business_intent"), b"order-123");
    assert_eq!(row.get::<u8, _>("end_operation"), 0);
    assert_eq!(row.get::<u8, _>("send_started"), 0);
    assert_eq!(row.get::<i8, _>("valid_deadline"), 1);
    for sql in [
        "INSERT INTO seata_rs_tm_launcher_v1 (schema_version) VALUES (1)",
        "UPDATE seata_rs_tm_launcher_v1 SET end_operation=1",
        "UPDATE seata_rs_tm_launcher_v1 SET send_started=1",
        "DELETE FROM seata_rs_tm_launcher_v1",
    ] {
        assert_launcher_permission_denied(&mut reader, sql).await;
    }
    for sql in [
        "UPDATE seata_rs_tm_launcher_v1 SET owner_epoch=2",
        "UPDATE seata_rs_tm_launcher_v1 SET owner_instance_id=NULL",
        "UPDATE seata_rs_tm_launcher_v1 SET owner_stop_scope=NULL",
        "INSERT INTO seata_rs_tm_launcher_v1 (owner_epoch) VALUES (2)",
        "INSERT INTO seata_rs_tm_launcher_v1 (send_started) VALUES (1)",
        "DELETE FROM seata_rs_tm_launcher_v1",
    ] {
        assert_launcher_permission_denied(&mut writer, sql).await;
    }
    tx.commit().await.unwrap();
    let completed = inspect
        .load_snapshot(&Xid::parse("tc:durable-real".into()).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.end_operation(), Some(TmOperation::Commit));
    assert!(completed.send_started());
    assert_eq!(completed.owner_epoch(), 1);
    assert!(!completed.deadline_passed());
    tm.shutdown().await.unwrap();
    let recovery = tm_fixture::manager(endpoint, Duration::from_secs(2));
    assert_ne!(completed.owner_instance_id(), recovery.owner_instance_id());
    assert!(matches!(
        completed.query_original_tc(&recovery, "other-order").await,
        Err(TmError::IllegalState { .. })
    ));
    for (address, application, group) in [
        ("127.0.0.1:0".parse().unwrap(), "tm-app", "tm-group"),
        (endpoint, "other-app", "tm-group"),
        (endpoint, "tm-app", "other-group"),
    ] {
        assert!(matches!(
            recovery
                .get_status_at_origin(completed.xid(), address, application, group)
                .await,
            Err(TmError::IllegalState { .. })
        ));
    }
    assert_eq!(
        completed
            .query_original_tc(&recovery, "order-123")
            .await
            .unwrap()
            .status(),
        GlobalStatus::Finished
    );
    sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET created_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 2 SECOND), deadline_at=DATE_SUB(UTC_TIMESTAMP(6),INTERVAL 1 SECOND)")
        .execute(&mut admin).await.unwrap();
    let expired = inspect
        .load_snapshot(&Xid::parse("tc:durable-real".into()).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert!(expired.deadline_passed());
    assert!(matches!(
        expired.query_original_tc(&recovery, "order-123").await,
        Err(TmError::TransactionDeadlineExceeded { .. })
    ));
    recovery.shutdown().await.unwrap();
    server.await.unwrap();
    data_source.close().await.unwrap();
    read_source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn snapshot_rejects_same_digest_with_different_full_xid() {
    let mut admin = owned_mysql::admin().await;
    apply_launcher_migrations(&mut admin).await;
    let data_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(data_source.clone());
    let xid = Xid::parse("tc:original".into()).unwrap();
    assert!(store.load_snapshot(&xid).await.unwrap().is_none());
    let digest = Sha256::digest(xid.as_str().as_bytes());
    sqlx::query("INSERT INTO seata_rs_tm_launcher_v1 (schema_version,xid_digest,xid,tc_endpoint,application_id,service_group,begin_exception_code,business_intent,owner_instance_id,deadline_at,created_at,modified_at) VALUES (1,?,?,?,?,?,0,?,?,DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 60 SECOND),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
        .bind(digest.as_slice()).bind(b"tc:forged".as_slice())
        .bind(b"127.0.0.1:8091".as_slice()).bind(b"tm-app".as_slice())
        .bind(b"tm-group".as_slice()).bind(b"order-123".as_slice())
        .bind([1_u8; 32].as_slice())
        .execute(&mut admin).await.unwrap();
    assert!(matches!(
        store.load_snapshot(&xid).await,
        Err(DbError::RecoveryConflict)
    ));
    data_source.close().await.unwrap();
}
