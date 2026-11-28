//! 真正 RM 出站和重复/反向回调进入正式 JSON 订阅器，不改变资源一次性效果。
#![cfg(feature = "structured_logs")]
#![allow(clippy::unwrap_used)]
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod fixture;
#[path = "support/structured_log_sink.rs"]
mod sink;
use seata_observability::{StructuredLogConfig, StructuredLogSubscriber};
use seata_protocol::{
    transaction::{
        BranchCommitRequest, BranchRollbackRequest, GlobalLockQueryRequest, GlobalLockQueryResponse,
    },
    BranchType, Message, RequestType, ResultCode,
};
use std::{sync::atomic::Ordering, time::Duration};
use tokio::net::TcpListener;
fn query() -> GlobalLockQueryRequest {
    GlobalLockQueryRequest {
        xid: Some("tc:log-rm".into()),
        branch_type: BranchType::At,
        resource_id: Some("mysql://exact-resource".into()),
        lock_key: Some("private-lock-key:1".into()),
        application_data: Some("private-application-data".into()),
    }
}
#[tokio::test]
async fn real_rm_rpc_keeps_digest_and_lost_reply_without_payloads() {
    let (subscriber, receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(16, 4096).unwrap());
    let _capture = tracing::subscriber::set_default(subscriber);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = fixture::manager(&listener, Duration::from_secs(2)).await;
    let client = rm.client();
    let server = tokio::spawn(async move {
        let mut socket = fixture::registered(&listener).await;
        let mut ids = Vec::new();
        for rejected in [false, true] {
            let request = fixture::peer::read(&mut socket).await;
            ids.push(request.id);
            let mut response = fixture::peer::success();
            if rejected {
                response.result_code = ResultCode::Failed;
                response.msg = Some("private-rm-tc-error".into());
            }
            fixture::peer::write(
                &mut socket,
                fixture::peer::reply(
                    request.id,
                    Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
                        response,
                        lockable: false,
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
    assert!(!client.lock_query(query()).await.unwrap().lockable);
    assert!(matches!(
        client.lock_query(query()).await,
        Err(seata_rm::RmCallError::DefiniteRejected { .. })
    ));
    assert!(client
        .lock_query(query())
        .await
        .unwrap_err()
        .is_unknown_outcome());
    rm.shutdown().await.unwrap();
    assert!(client.lock_query(query()).await.is_err());
    let ids = server.await.unwrap();
    let records=sink::finish(receiver,&sampler,"rm-native",serde_json::json!({"actual_request_ids":ids,"resource":"mysql://exact-resource","xid":"tc:log-rm"})).await;
    assert_eq!(records.len(), 4);
    for (index, event) in records.iter().enumerate() {
        assert_eq!(event["fields"]["operation"], "LockQuery");
        assert_eq!(
            event["fields"]["outcome"],
            match index {
                0 | 1 => "response",
                2 => "unknown",
                _ => "not_sent",
            }
        );
        let text = event.to_string();
        for secret in [
            "private-lock-key",
            "private-application-data",
            "private-rm-tc-error",
            "mysql://exact-resource",
        ] {
            assert!(!text.contains(secret));
        }
    }
}
#[tokio::test]
async fn real_duplicate_and_opposite_callbacks_keep_ids_generations_and_one_effect() {
    let (subscriber, receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(16, 4096).unwrap());
    let _capture = tracing::subscriber::set_default(subscriber);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = fixture::manager(&listener, Duration::from_secs(2)).await;
    let mut socket = fixture::registered(&listener).await;
    let branch = 9_007_199_254_740_995_i64;
    let mut responses = Vec::new();
    for (id, commit, status) in [(i32::MIN, true, 5), (-11, true, 5), (-12, false, 10)] {
        let body = if commit {
            Message::BranchCommitRequest(BranchCommitRequest {
                xid: "tc:log-callback".into(),
                branch_id: branch,
                branch_type: BranchType::At,
                resource_id: "mysql://exact-resource".into(),
                application_data: Some("private-callback-data".into()),
            })
        } else {
            Message::BranchRollbackRequest(BranchRollbackRequest {
                xid: "tc:log-callback".into(),
                branch_id: branch,
                branch_type: BranchType::At,
                resource_id: "mysql://exact-resource".into(),
                application_data: Some("private-callback-data".into()),
            })
        };
        let mut frame = fixture::peer::reply(id, body);
        frame.message_type = RequestType::RequestSync;
        fixture::peer::write(&mut socket, frame).await;
        let response = fixture::peer::read(&mut socket).await;
        assert_eq!(response.id, id);
        let branch_status = match response.body.unwrap() {
            Message::BranchCommitResponse(v) => v.branch_status.to_java_v1(),
            Message::BranchRollbackResponse(v) => v.branch_status.to_java_v1(),
            _ => panic!("wrong callback response"),
        };
        assert_eq!(branch_status, status);
        responses.push(serde_json::json!({"id":response.id,"branch_status_code":branch_status}));
    }
    rm.shutdown().await.unwrap();
    assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
    let records=sink::finish(receiver,&sampler,"rm-callback",serde_json::json!({"responses":responses,"branch_id":branch.to_string(),"resource":"mysql://exact-resource","actual_effects":resource.actions.load(Ordering::SeqCst)})).await;
    assert_eq!(records.len(), 6);
    for response in &responses {
        let id = response["id"].as_i64().unwrap().to_string();
        let matching: Vec<_> = records
            .iter()
            .filter(|event| event["fields"]["request_id"] == id)
            .collect();
        assert_eq!(matching.len(), 2);
        assert_eq!(matching[0]["fields"]["branch_id"], branch.to_string());
        assert_eq!(
            matching[0]["fields"]["connection_generation"],
            matching[1]["fields"]["connection_generation"]
        );
        assert_eq!(matching[1]["fields"]["outcome"], "local_written");
    }
}
