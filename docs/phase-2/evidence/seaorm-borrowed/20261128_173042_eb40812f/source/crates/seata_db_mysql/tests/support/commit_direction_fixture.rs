//! 自有提交方向夹具；真实协议注册与可指定文件分支日志。
use super::{first_fixture, rm_fixture};
use first_fixture::Harness;
use seata_protocol::transaction::BranchRegisterRequest;
use seata_protocol::Message;
use seata_rm::{BranchResource, RegisteredBranch};
use std::time::Duration;
use tokio::net::TcpStream;

pub(super) async fn enrolled() -> (Harness, RegisteredBranch, TcpStream) {
    enrolled_at(3, Duration::from_secs(5)).await
}
pub(super) async fn enrolled_at(
    schema: u8,
    budget: Duration,
) -> (Harness, RegisteredBranch, TcpStream) {
    enrolled_with_store(schema, budget, None).await
}
pub(super) async fn enrolled_with_store(
    schema: u8,
    budget: Duration,
    store: Option<seata_rm::FileBranchStore>,
) -> (Harness, RegisteredBranch, TcpStream) {
    let mut h = if let Some(store) = store {
        Harness::start_with_store(budget, Duration::from_secs(2), store).await
    } else {
        Harness::start(budget, Duration::from_secs(2)).await
    };
    if schema == 1 {
        sqlx::query("ALTER TABLE seata_rs_at_branch_v1 DROP COLUMN registration_origin")
            .execute(&mut h.admin)
            .await
            .unwrap();
    } else if schema == 3 {
        sqlx::raw_sql(include_str!(
            "../../migrations/003_seata_rs_at_commit_decision.sql"
        ))
        .execute(&mut h.admin)
        .await
        .unwrap();
    }
    let request = BranchRegisterRequest {
        xid: Some("rollback:xid".into()),
        branch_type: h.adapter.key().branch_type(),
        resource_id: Some(h.adapter.key().resource_id().into()),
        application_data: None,
        lock_key: Some("undo_work:1".into()),
    };
    let client = h.rm.client();
    let server = async {
        let mut socket = h.accept().await;
        let value = rm_fixture::peer::read(&mut socket).await;
        assert!(matches!(
            value.body,
            Some(Message::BranchRegisterRequest(_))
        ));
        first_fixture::positive(&mut socket, value.id).await;
        socket
    };
    let (receipt, socket) = tokio::join!(client.branch_register_with_receipt(request), server);
    let receipt = receipt.unwrap();
    h.adapter.enroll(&receipt).await.unwrap();
    (h, receipt, socket)
}
