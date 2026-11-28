#![allow(clippy::unwrap_used, dead_code)]
#[path = "../../../seata_transport/tests/support/session_peer.rs"]
pub mod peer;
use seata_protocol::protocol::RegisterTMResponse;
use seata_protocol::transaction::{
    AbstractTransactionResponse, GlobalBeginResponse, GlobalCommitResponse, GlobalRollbackResponse,
};
use seata_protocol::{GlobalStatus, Message};
use seata_tm::{TransactionConfig, TransactionManager};
use seata_transport::{ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
pub fn settings(endpoint: std::net::SocketAddr) -> ConnectionPoolConfig {
    ConnectionPoolConfig::new(
        endpoint,
        peer::config(8),
        1,
        4,
        ReconnectPolicy::new(
            Duration::from_millis(20),
            Duration::from_millis(80),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn identity() -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        "tm-app".into(),
        "tm-group".into(),
        Some("public-fixture".into()),
    )
    .unwrap()
}
pub fn manager(endpoint: std::net::SocketAddr, timeout: Duration) -> TransactionManager {
    TransactionManager::builder(settings(endpoint), identity())
        .request_timeout(timeout)
        .build()
        .unwrap()
}
pub fn config() -> TransactionConfig {
    TransactionConfig::builder("business".into())
        .transaction_timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}
pub async fn registered(listener: &TcpListener) -> TcpStream {
    let (mut socket, _) = listener.accept().await.unwrap();
    let request = peer::read(&mut socket).await;
    match request.body {
        Some(Message::RegisterTMRequest(value)) => {
            assert_eq!(value.application_id, "tm-app");
            assert_eq!(value.transaction_service_group, "tm-group");
            assert_eq!(value.extra_data, "public-fixture");
        }
        other => panic!("{other:?}"),
    }
    peer::write(
        &mut socket,
        peer::reply(
            request.id,
            Message::RegisterTMResponse(RegisterTMResponse {
                identified: true,
                wire_version: None,
            }),
        ),
    )
    .await;
    socket
}
pub async fn begin(socket: &mut TcpStream, value: Option<&str>) {
    let request = peer::read(socket).await;
    match request.body {
        Some(Message::GlobalBeginRequest(value)) => {
            assert_eq!(value.timeout, 30000);
            assert_eq!(value.transaction_name.as_deref(), Some("business"));
        }
        other => panic!("{other:?}"),
    }
    peer::write(
        socket,
        peer::reply(
            request.id,
            Message::GlobalBeginResponse(GlobalBeginResponse {
                response: peer::success(),
                xid: value.map(str::to_owned),
                extra_data: Some("server-extra".into()),
            }),
        ),
    )
    .await;
}
pub fn end_response(
    commit: bool,
    status: GlobalStatus,
    response: AbstractTransactionResponse,
) -> Message {
    if commit {
        Message::GlobalCommitResponse(GlobalCommitResponse {
            response,
            global_status: status,
        })
    } else {
        Message::GlobalRollbackResponse(GlobalRollbackResponse {
            response,
            global_status: status,
        })
    }
}
