//! 真 TCP 注册回执与显式恢复表迁移；不把测试对端称为原 Java TC。
use super::{owned_mysql, rm_fixture};
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_protocol::protocol::RegisterRMResponse;
use seata_protocol::transaction::{BranchRegisterRequest, BranchRegisterResponse};
use seata_protocol::Message;
use seata_rm::{BranchResource, RegisteredBranch, ResourceCatalog, ResourceManager};
use sqlx::MySqlConnection;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

pub async fn schema(admin: &mut MySqlConnection) {
    sqlx::query("DROP TABLE IF EXISTS seata_rs_at_branch_v1")
        .execute(&mut *admin)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/001_seata_rs_at_branch_v1.sql"
    ))
    .execute(admin)
    .await
    .unwrap();
}
pub fn adapter(ds: MySqlDataSource) -> Arc<MySqlDataSourceManager> {
    Arc::new(MySqlDataSourceManager::new(ds, MySqlJdbcProfile::Connector8Default).unwrap())
}
pub async fn receipt(adapter: Arc<MySqlDataSourceManager>, app: Option<&str>) -> RegisteredBranch {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let key = adapter.key().clone();
    let expected_key = key.clone();
    let rm = ResourceManager::builder(
        rm_fixture::settings(listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        rm_fixture::journal().await,
    )
    .request_timeout(Duration::from_secs(2))
    .build()
    .unwrap();
    let request = BranchRegisterRequest {
        xid: Some("rollback:xid".into()),
        branch_type: key.branch_type(),
        resource_id: Some(key.resource_id().into()),
        application_data: app.map(str::to_owned),
        lock_key: Some("undo_work:1".into()),
    };
    let expected = request.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let register = rm_fixture::peer::read(&mut socket).await;
        match register.body {
            Some(Message::RegisterRMRequest(v)) => {
                assert_eq!(v.resource_ids.as_deref(), Some(expected_key.resource_id()))
            }
            other => panic!("{other:?}"),
        };
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                register.id,
                Message::RegisterRMResponse(RegisterRMResponse {
                    identified: true,
                    wire_version: None,
                }),
            ),
        )
        .await;
        let branch = rm_fixture::peer::read(&mut socket).await;
        assert_eq!(branch.body, Some(Message::BranchRegisterRequest(expected)));
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                branch.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: rm_fixture::peer::success(),
                    branch_id: 42,
                }),
            ),
        )
        .await;
        rm_fixture::peer::eof(&mut socket).await;
    });
    let receipt = rm
        .client()
        .branch_register_with_receipt(request)
        .await
        .unwrap();
    rm.shutdown().await.unwrap();
    server.await.unwrap();
    receipt
}
pub async fn empty() -> (MySqlConnection, Arc<MySqlDataSourceManager>) {
    let mut admin = super::fixture::reset().await;
    schema(&mut admin).await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    (admin, adapter(ds))
}
pub async fn state(admin: &mut MySqlConnection) -> (i64, i64, i64) {
    sqlx::query_as("SELECT COUNT(*),CAST(COALESCE(SUM(phase),0) AS SIGNED),CAST(COALESCE(SUM(terminal),0) AS SIGNED) FROM seata_rs_at_branch_v1").fetch_one(admin).await.unwrap()
}
