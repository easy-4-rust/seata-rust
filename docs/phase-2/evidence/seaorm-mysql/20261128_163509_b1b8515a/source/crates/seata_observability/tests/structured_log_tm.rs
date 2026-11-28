//! 真正 TM TCP 成功、明确拒绝和响应丢失进入正式 JSON 订阅器。
#![cfg(feature = "structured_logs")]
#![allow(clippy::unwrap_used)]
#[path = "../../seata_tm/tests/support/tm_peer.rs"]
mod fixture;
#[path = "support/structured_log_sink.rs"]
mod sink;
use seata_observability::{StructuredLogConfig, StructuredLogSubscriber};
use seata_protocol::{transaction::GlobalStatusResponse, GlobalStatus, Message, ResultCode};
use seata_tm::{TmError, Xid};
use std::time::Duration;
use tokio::net::TcpListener;
#[tokio::test]
async fn real_tm_response_rejection_and_loss_keep_original_request_correlations() {
    let (subscriber, receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(16, 4096).unwrap());
    let _capture = tracing::subscriber::set_default(subscriber);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = fixture::manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = fixture::registered(&listener).await;
        let mut ids = Vec::new();
        for rejected in [false, true] {
            let request = fixture::peer::read(&mut socket).await;
            assert!(matches!(
                request.body,
                Some(Message::GlobalStatusRequest(_))
            ));
            ids.push(request.id);
            let mut response = fixture::peer::success();
            if rejected {
                response.result_code = ResultCode::Failed;
                response.msg = Some("private-tc-response-credential".into());
            }
            fixture::peer::write(
                &mut socket,
                fixture::peer::reply(
                    request.id,
                    Message::GlobalStatusResponse(GlobalStatusResponse {
                        response,
                        global_status: GlobalStatus::Rollbacking,
                    }),
                ),
            )
            .await;
        }
        let request = fixture::peer::read(&mut socket).await;
        ids.push(request.id);
        drop(socket);
        ids
    });
    let xid = Xid::parse("tc:log-native".into()).unwrap();
    assert_eq!(
        tm.get_status(&xid).await.unwrap().status(),
        GlobalStatus::Rollbacking
    );
    assert!(matches!(
        tm.get_status(&xid).await,
        Err(TmError::DefiniteRejected { .. })
    ));
    assert!(tm.get_status(&xid).await.unwrap_err().is_unknown_outcome());
    tm.shutdown().await.unwrap();
    let ids = server.await.unwrap();
    let records=sink::finish(receiver,&sampler,"tm-native",serde_json::json!({"actual_request_ids":ids,"xid":xid.as_str(),"responses":["Rollbacking","TC rejected","reply lost"]})).await;
    assert_eq!(records.len(), 3);
    for (index, event) in records.iter().enumerate() {
        assert_eq!(event["target"], "seata_tm::rpc");
        assert_eq!(event["fields"]["operation"], "Status");
        assert_eq!(event["fields"]["xid"], "Some(\"tc:log-native\")");
        assert_eq!(
            event["fields"]["outcome"],
            if index == 2 { "unknown" } else { "response" }
        );
        assert_eq!(
            event["fields"]["request_id"],
            if index == 2 {
                format!("Some({})", ids[index])
            } else {
                ids[index].to_string()
            }
        );
        assert!(!event.to_string().contains("private-tc-response-credential"));
    }
}
