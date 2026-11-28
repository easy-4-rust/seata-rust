//! 真实 MySQL 中原 Fastjson 与 Fastjson2 根行及子片的受控补偿验收。
#![allow(clippy::unwrap_used)]
use super::MySqlDataSource;
use crate::{
    undo::UndoSerializer, DbError, MySqlOptions, UndoRollbackError, UndoRollbackObservation,
};
use seata_at::MySqlJdbcProfile;
use sqlx::{Connection, MySqlConnection};

const FASTJSON: &[u8] = include_bytes!("../../seata_at/tests/fixtures/fastjson_native_wire.txt");
const FASTJSON2: &[u8] = include_bytes!("../../seata_at/tests/fixtures/fastjson2_native_wire.bin");
const XID: &str = "fastjson-native:42";
const PROFILE: MySqlJdbcProfile = MySqlJdbcProfile::Connector8NumericTinyInt;

#[tokio::test]
#[ignore = "requires owned real MySQL with original undo_log schema"]
async fn fastjson_split_undo_validates_all_logs_before_writes_and_then_cleans_up() {
    assert_split_undo(UndoSerializer::Fastjson, "fastjson", FASTJSON).await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL with original undo_log schema"]
async fn fastjson2_split_undo_validates_all_logs_before_writes_and_then_cleans_up() {
    assert_split_undo(UndoSerializer::Fastjson2, "fastjson2", FASTJSON2).await;
}

async fn assert_split_undo(format: UndoSerializer, serializer: &str, wire: &[u8]) {
    let database = std::env::var("SEATA_MYSQL_DATABASE").unwrap();
    let owner = database.strip_prefix("undo_").unwrap();
    assert!(owner.len() == 12 && owner.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let port = std::env::var("SEATA_MYSQL_PORT")
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let password = std::env::var("SEATA_MYSQL_PASSWORD").unwrap();
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        port,
        "root".into(),
        password.clone(),
        database.clone(),
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap();
    let mut admin = MySqlConnection::connect_with(
        &sqlx::mysql::MySqlConnectOptions::new()
            .host("127.0.0.1")
            .port(port)
            .username("root")
            .password(&password)
            .database(&database)
            .charset("utf8mb4")
            .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled),
    )
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE undo_fixture (id BIGINT PRIMARY KEY, small INT NOT NULL, wide BIGINT NOT NULL, \
         `unsigned` BIGINT UNSIGNED NOT NULL, `decimal` DECIMAL(20,10) NOT NULL, whole DECIMAL(20,0) NOT NULL, \
         `text` VARCHAR(32) NOT NULL, bytes VARBINARY(128) NOT NULL, nullable VARCHAR(32)) \
         ENGINE=InnoDB DEFAULT CHARACTER SET utf8mb4",
    ).execute(&mut admin).await.unwrap();

    // 最早的 SQL 日志在反序补偿时最后执行，破坏其列名可检验全部日志先绑定。
    let mut damaged = wire.to_vec();
    let target = damaged
        .windows(5)
        .position(|window| window == b"small")
        .unwrap();
    damaged[target..target + 5].copy_from_slice(b"other");
    let midpoint = wire.len() / 2;
    let context = format!("serializer={serializer}&compressorType=NONE&map=67108864&subId=43,");
    let mut seed = admin.begin().await.unwrap();
    for (id, context, bytes) in [
        (42, context.as_str(), &damaged[..midpoint]),
        (43, "branchId=42", &damaged[midpoint..]),
    ] {
        sqlx::query("INSERT INTO undo_log VALUES(?,?,?,?,0,now(6),now(6))")
            .bind(id)
            .bind(XID)
            .bind(context)
            .bind(bytes)
            .execute(&mut *seed)
            .await
            .unwrap();
    }
    seed.commit().await.unwrap();

    let source = MySqlDataSource::connect(options).await.unwrap();
    let before = dml_counters(&mut admin).await;
    assert!(matches!(
        source.rollback_undo_log(XID, 42, PROFILE).await,
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::Unsupported { .. },
            ..
        })
    ));
    assert!(matches!(
        source
            .rollback_undo_log_for_serializer(XID, 42, PROFILE, format)
            .await,
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::UndoCodec(_),
            ..
        })
    ));
    let after_rejection = dml_counters(&mut admin).await;
    assert_eq!(after_rejection, before);
    assert_counts(&mut admin, 2).await;

    let mut repair = admin.begin().await.unwrap();
    for (id, bytes) in [(42, &wire[..midpoint]), (43, &wire[midpoint..])] {
        let affected =
            sqlx::query("UPDATE undo_log SET rollback_info=? WHERE xid=? AND branch_id=?")
                .bind(bytes)
                .bind(XID)
                .bind(id)
                .execute(&mut *repair)
                .await
                .unwrap()
                .rows_affected();
        assert_eq!(affected, 1);
    }
    repair.commit().await.unwrap();

    assert_eq!(
        source
            .rollback_undo_log_for_serializer(XID, 42, PROFILE, format)
            .await
            .unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 3,
            restored_rows: 3
        }
    );
    assert_counts(&mut admin, 0).await;
    if let Some(output) = std::env::var_os("SEATA_UNDO_STORE_OUT") {
        let evidence = serde_json::json!({
            "xid": XID, "branch_id": 42, "child_branch_id": 43, "serializer": serializer,
            "dml_before_attempts": before, "dml_after_rejection": after_rejection,
            "restored_operations": 3, "remaining_business_rows": 0, "remaining_undo_rows": 0
        });
        std::fs::write(
            std::path::PathBuf::from(output).join(format!("{serializer}-control-transaction.json")),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
    }
    source.close().await.unwrap();
}

async fn assert_counts(admin: &mut MySqlConnection, undo: i64) {
    let business = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM undo_fixture")
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    let logs = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM undo_log WHERE xid=?")
        .bind(XID)
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    assert_eq!((business, logs), (0, undo));
}

async fn dml_counters(admin: &mut MySqlConnection) -> Vec<(String, String)> {
    let mut rows = sqlx::query_as::<_, (String, String)>(
        "SHOW GLOBAL STATUS WHERE Variable_name IN ('Com_insert','Com_update','Com_delete')",
    )
    .fetch_all(admin)
    .await
    .unwrap();
    rows.sort_unstable();
    assert_eq!(rows.len(), 3);
    rows
}
