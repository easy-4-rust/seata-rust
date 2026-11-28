//! 自有 MySQL TCC 结构、正式 RM 配置和测试业务动作。
#![allow(clippy::unwrap_used, dead_code)]
#[path = "owned_mysql.rs"]
pub mod owned_mysql;
use seata_datasource::FieldValue;
use seata_db_mysql::{MySqlDataSource, MySqlTccResourceManager};
use seata_tcc::{BusinessActionContext, TccBusinessAction, TccError, TccTransaction};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
};
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc, time::Duration};
/// 无参数；显式重建独立 TCC 表，返回原观察连接、数据源和资源。
pub async fn seed() -> (
    sqlx::MySqlConnection,
    MySqlDataSource,
    Arc<MySqlTccResourceManager>,
) {
    let mut admin = owned_mysql::admin().await;
    sqlx::raw_sql("DROP TABLE IF EXISTS seata_rs_tcc_fence_v1; DROP TABLE IF EXISTS tcc_managed; CREATE TABLE tcc_managed(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB").execute(&mut admin).await.unwrap();
    sqlx::raw_sql(include_str!("../../migrations/019_seata_rs_tcc_fence.sql"))
        .execute(&mut admin)
        .await
        .unwrap();
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let resource = Arc::new(
        MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            Arc::new(Action::default()),
        )
        .unwrap(),
    );
    (admin, source, resource)
}
/// 明确同库 DML 的测试业务；失败开关仅影响返回值，不绕过生产事务。
#[derive(Debug, Default)]
pub struct Action {
    pub fail_try: bool,
    pub fail_confirm: bool,
    pub poison: bool,
    pub entered: Option<Arc<tokio::sync::Notify>>,
    pub release: Option<Arc<tokio::sync::Notify>>,
}
impl Action {
    async fn invoke(
        &self,
        phase: u8,
        context: &BusinessActionContext,
        transaction: &mut dyn TccTransaction,
    ) -> Result<bool, TccError> {
        let id: i64 = serde_json::from_str(
            context
                .get_action_context("id")
                .ok_or(TccError::Business)?
                .get(),
        )
        .map_err(|_| TccError::Business)?;
        let value: i64 = serde_json::from_str(
            context
                .get_action_context("amount")
                .ok_or(TccError::Business)?
                .get(),
        )
        .map_err(|_| TccError::Business)?;
        let (sql, parameters) = match phase {
            0 => (
                "INSERT INTO tcc_managed(id,v) VALUES(?,?)",
                vec![FieldValue::Signed(id), FieldValue::Signed(value)],
            ),
            1 => (
                "UPDATE tcc_managed SET v=? WHERE id=?",
                vec![FieldValue::Signed(value * 2), FieldValue::Signed(id)],
            ),
            _ => (
                "DELETE FROM tcc_managed WHERE id=?",
                vec![FieldValue::Signed(id)],
            ),
        };
        transaction.execute(sql, &parameters).await?;
        if phase == 0 {
            if let Some(entered) = &self.entered {
                entered.notify_one();
            }
            if let Some(release) = &self.release {
                release.notified().await;
            }
        }
        if self.poison {
            let _ = transaction
                .execute("UPDATE seata_rs_tcc_fence_v1 SET status=2", &[])
                .await;
        }
        Ok(!(phase == 0 && self.fail_try || phase == 1 && self.fail_confirm))
    }
}
impl TccBusinessAction for Action {
    fn try_action<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(self.invoke(0, context, transaction))
    }
    fn confirm<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(self.invoke(1, context, transaction))
    }
    fn cancel<'a>(
        &'a self,
        context: &'a BusinessActionContext,
        transaction: &'a mut dyn TccTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(self.invoke(2, context, transaction))
    }
}
/// 参数为业务编号；返回未经修改传递给原 TC 的应用信封。
pub fn app(id: i64) -> String {
    format!("{{\"actionContext\":{{\"useTCCFence\":true,\"id\":{id},\"amount\":7}}}}")
}
/// 参数为自有回环端点；返回原 V1 帧预算及有界单槽重连配置。
pub fn pool(endpoint: std::net::SocketAddr) -> ConnectionPoolConfig {
    let connection = ConnectionConfig::new(
        seata_protocol::v1::FrameLimits::new(
            65536,
            32767,
            128,
            seata_protocol::PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
        )
        .unwrap(),
        16,
        Duration::from_secs(3),
        Duration::from_secs(3),
    )
    .unwrap();
    ConnectionPoolConfig::new(
        endpoint,
        connection,
        1,
        16,
        ReconnectPolicy::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
/// 参数为是否连接固定原 Java TC；返回匹配该隔离夹具的原登记身份，不更改生产认证。
pub fn registration(java: bool) -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        if java { "phase0-auth" } else { "rm-app" }.into(),
        if java { "default_tx_group" } else { "rm-group" }.into(),
        Some(
            if java {
                "phase0-local-only"
            } else {
                "rm-public-fixture"
            }
            .into(),
        ),
    )
    .unwrap()
}
/// 参数为固定证据名；返回仅位于 D/E 的自有输出目录，禁止写开发工作区外其他盘。
pub fn folder(name: &str) -> PathBuf {
    let out = PathBuf::from(std::env::var_os("SEATA_XA_OUT").unwrap());
    assert!(
        out.is_absolute()
            && (out.to_string_lossy().starts_with("D:") || out.to_string_lossy().starts_with("E:"))
    );
    let path = out.join(name);
    std::fs::create_dir_all(&path).unwrap();
    path
}
/// 参数为固定名称与实际观察；记录当前原进程 ID 和结构化现场，不替代独立 SQL。
pub fn record(name: &str, mut value: serde_json::Value) {
    value.as_object_mut().unwrap().insert(
        "rust_process_id".into(),
        serde_json::json!(std::process::id()),
    );
    std::fs::write(
        folder("proof").join(format!("{name}.json")),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}
/// 参数为原独立连接；返回按原主键排序的真实业务值。
pub async fn business(admin: &mut sqlx::MySqlConnection) -> Vec<(i32, i32)> {
    sqlx::query_as("SELECT id,v FROM tcc_managed ORDER BY id")
        .fetch_all(admin)
        .await
        .unwrap()
}
