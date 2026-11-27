//! 正式 RM 的原登记和回调测试对端，不代替资源数据库逻辑。
#![allow(clippy::unwrap_used, dead_code)]
#[path = "../../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;
use rm_fixture::peer;
use seata_protocol::{
    protocol::RegisterRMResponse,
    transaction::{BranchCommitRequest, BranchRegisterResponse, BranchRollbackRequest},
    BranchType, Message, RequestType, ResultCode,
};
use seata_rm::{BranchIdentity, BranchPhase};
use tokio::net::{TcpListener, TcpStream};
pub async fn bootstrap(listener: &TcpListener, resource: &str) -> TcpStream {
    let (mut socket, _) = listener.accept().await.unwrap();
    let request = peer::read(&mut socket).await;
    let Some(Message::RegisterRMRequest(fields)) = request.body else {
        panic!("missing original RM registration")
    };
    assert_eq!(fields.resource_ids.as_deref(), Some(resource));
    assert_eq!(fields.application_id.as_deref(), Some("rm-app"));
    peer::write(
        &mut socket,
        peer::reply(
            request.id,
            Message::RegisterRMResponse(RegisterRMResponse {
                identified: true,
                wire_version: None,
            }),
        ),
    )
    .await;
    socket
}
pub async fn registered(
    socket: &mut TcpStream,
    resource: &str,
    xid: &str,
    app: Option<&str>,
    branch: i64,
) {
    let id = read_registration(socket, resource, xid, app).await;
    reply_registered(socket, id, branch).await;
}
pub async fn read_registration(
    socket: &mut TcpStream,
    resource: &str,
    xid: &str,
    app: Option<&str>,
) -> i32 {
    let request = peer::read(socket).await;
    let Some(Message::BranchRegisterRequest(fields)) = request.body else {
        panic!("TCC branch registration missing")
    };
    assert_eq!(fields.branch_type, BranchType::Tcc);
    assert_eq!(fields.resource_id.as_deref(), Some(resource));
    assert_eq!(fields.xid.as_deref(), Some(xid));
    assert_eq!(fields.application_data.as_deref(), app);
    assert_eq!(fields.lock_key, None);
    request.id
}
pub async fn reply_registered(socket: &mut TcpStream, id: i32, branch: i64) {
    peer::write(
        socket,
        peer::reply(
            id,
            Message::BranchRegisterResponse(BranchRegisterResponse {
                response: peer::success(),
                branch_id: branch,
            }),
        ),
    )
    .await;
}

pub async fn callback(
    socket: &mut TcpStream,
    id: i32,
    identity: &BranchIdentity,
    phase: BranchPhase,
) -> (ResultCode, u8) {
    let body = match phase {
        BranchPhase::Commit => Message::BranchCommitRequest(BranchCommitRequest {
            xid: identity.xid().into(),
            branch_id: identity.branch_id(),
            branch_type: BranchType::Tcc,
            resource_id: identity.resource().resource_id().into(),
            application_data: identity.application_data().map(str::to_owned),
        }),
        BranchPhase::Rollback => Message::BranchRollbackRequest(BranchRollbackRequest {
            xid: identity.xid().into(),
            branch_id: identity.branch_id(),
            branch_type: BranchType::Tcc,
            resource_id: identity.resource().resource_id().into(),
            application_data: identity.application_data().map(str::to_owned),
        }),
    };
    let mut frame = peer::reply(id, body);
    frame.message_type = RequestType::RequestSync;
    peer::write(socket, frame).await;
    let response = peer::read(socket).await;
    assert_eq!(response.id, id);
    match response.body.unwrap() {
        Message::BranchCommitResponse(value) => {
            assert_eq!(value.branch_id, identity.branch_id());
            (value.response.result_code, value.branch_status.to_java_v1())
        }
        Message::BranchRollbackResponse(value) => {
            assert_eq!(value.branch_id, identity.branch_id());
            (value.response.result_code, value.branch_status.to_java_v1())
        }
        other => panic!("unexpected callback response: {other:?}"),
    }
}
