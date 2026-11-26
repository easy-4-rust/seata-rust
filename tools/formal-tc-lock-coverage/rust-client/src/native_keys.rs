use seata_at::{ConnectionContext, JacksonUndoLogParser, MySqlJdbcProfile, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use seata_db_mysql::{MySqlDataSource, MySqlOptions};
use serde_json::{json, Value};
use std::{error::Error, fs, path::Path};

/// 从真实MySQL执行镜像生成原格式锁键，保存日志后全部回滚；不把这些分离步骤声称为AT原子事务。
/// 对应 Java: BaseTransactionalExecutor#buildLockKey（验证驱动）。
pub async fn prepare(folder: &Path) -> Result<Vec<Value>, Box<dyn Error>> {
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT")?.parse()?,
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD")?,
        std::env::var("SEATA_MYSQL_DATABASE")?,
    )?
    .with_insecure_loopback_tls()?;
    let ds = MySqlDataSource::connect(options).await?;
    let text = |value: &str| FieldValue::Text(value.into());
    let one = |value: &str| vec![text(value)];
    let specs = [
        ("normal", "t", one("1"), one("1")),
        ("empty", "t", one(""), one("")),
        ("blank", "t", one(" "), one(" ")),
        ("unicode-blank", "t", one("\u{2003}"), one("\u{2003}")),
        ("partial-empty", "t", vec![text(""), text("1")], one("")),
        (
            "semicolon",
            "t",
            vec![text("x;broken"), text("z")],
            one("z"),
        ),
        (
            "binary-same",
            "t_binary",
            vec![FieldValue::Binary(vec![0, 255])],
            vec![FieldValue::Binary(vec![0, 255])],
        ),
        (
            "binary-distinct",
            "t_binary",
            vec![FieldValue::Binary(vec![0, 255])],
            vec![FieldValue::Binary(vec![0, 254])],
        ),
    ];
    let mut cases = Vec::new();
    for (index, (name, table, owner, contender)) in specs.into_iter().enumerate() {
        let mut keys = Vec::new();
        for (role, values) in [("owner", owner), ("contender", contender)] {
            let sql = format!(
                "INSERT INTO {table}(id) VALUES{}",
                vec!["(?)"; values.len()].join(",")
            );
            let mut tx = ds.begin().await?;
            let affected = tx
                .execute(&SqlRecognizerFactory::recognize(&sql)?, &values)
                .await?;
            super::need(
                affected == values.len() as u64,
                "actual prepared row count differs",
            )?;
            let context = ConnectionContext::from_undo_logs(
                tx.undo_logs(),
                MySqlJdbcProfile::Connector8Default,
            )?;
            keys.push(
                context
                    .build_lock_keys()
                    .ok_or_else(|| std::io::Error::other("prepared lock key missing"))?,
            );
            fs::write(
                folder.join(format!("native-{index}-{role}.json")),
                JacksonUndoLogParser::default().encode_prepared(
                    "coverage-provenance:xid",
                    42,
                    tx.undo_logs(),
                    MySqlJdbcProfile::Connector8Default,
                )?,
            )?;
            tx.rollback().await?;
        }
        cases.push(json!({"case":name,"owner_key":keys[0],"contender_key":keys[1]}));
    }
    ds.close().await?;
    fs::write(
        folder.join("native-cases.json"),
        serde_json::to_vec_pretty(&cases)?,
    )?;
    Ok(cases)
}
