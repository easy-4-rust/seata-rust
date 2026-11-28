//! 受控 TCP 回调夹具；仅模拟所配置对端，不声称是原 Java TC。
use super::rm_fixture::peer;
use seata_protocol::protocol::RegisterRMResponse;
use seata_protocol::transaction::{BranchCommitRequest, BranchRollbackRequest};
use seata_protocol::{BranchStatus, Message, RequestType, ResultCode};
use seata_rm::BranchPhase;
use seata_transport::ResourceKey;
use tokio::net::{TcpListener, TcpStream};

pub async fn registered(listener: &TcpListener, key: &ResourceKey) -> TcpStream {
    let (mut socket, _) = listener.accept().await.unwrap();
    let register = peer::read(&mut socket).await;
    match register.body {
        Some(Message::RegisterRMRequest(value)) => {
            assert_eq!(value.resource_ids.as_deref(), Some(key.resource_id()))
        }
        other => panic!("{other:?}"),
    }
    peer::write(
        &mut socket,
        peer::reply(
            register.id,
            Message::RegisterRMResponse(RegisterRMResponse {
                identified: true,
                wire_version: None,
            }),
        ),
    )
    .await;
    socket
}

pub async fn callback(
    socket: &mut TcpStream,
    key: &ResourceKey,
    xid: &str,
    branch: i64,
    wire: i32,
    app: Option<&str>,
    phase: BranchPhase,
) -> (ResultCode, BranchStatus) {
    let body = match phase {
        BranchPhase::Rollback => Message::BranchRollbackRequest(BranchRollbackRequest {
            xid: xid.into(),
            branch_id: branch,
            branch_type: key.branch_type(),
            resource_id: key.resource_id().into(),
            application_data: app.map(str::to_owned),
        }),
        BranchPhase::Commit => Message::BranchCommitRequest(BranchCommitRequest {
            xid: xid.into(),
            branch_id: branch,
            branch_type: key.branch_type(),
            resource_id: key.resource_id().into(),
            application_data: app.map(str::to_owned),
        }),
    };
    let mut request = peer::reply(wire, body);
    request.message_type = RequestType::RequestSync;
    peer::write(socket, request).await;
    let response = peer::read(socket).await;
    assert_eq!(response.id, wire);
    match response.body {
        Some(Message::BranchRollbackResponse(value)) => {
            assert_eq!(value.xid.as_deref(), Some(xid));
            assert_eq!(value.branch_id, branch);
            (value.response.result_code, value.branch_status)
        }
        Some(Message::BranchCommitResponse(value)) => {
            assert_eq!(value.xid.as_deref(), Some(xid));
            assert_eq!(value.branch_id, branch);
            (value.response.result_code, value.branch_status)
        }
        other => panic!("{other:?}"),
    }
}
