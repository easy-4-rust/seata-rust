use super::{fixture, EventCapture};
use fixture::{peer, registered};
use seata_protocol::protocol::MergedWarpMessage;
use seata_protocol::transaction::BranchCommitRequest;
use seata_protocol::{BranchType, Message, RequestType};
use seata_rm::{ResourceCatalog, ResourceManager};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;

fn request() -> Message {
    Message::BranchCommitRequest(BranchCommitRequest {
        xid: "tc:write-event".into(),
        branch_id: 103,
        branch_type: BranchType::At,
        resource_id: "mysql://exact-resource".into(),
        application_data: Some("write-event-secret".into()),
    })
}

#[tokio::test(flavor = "current_thread")]
async fn flat_duplicate_batch_has_two_decisions_one_write_and_one_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = fixture::manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(captured.clone()));
        let mut frame = peer::reply(
            -112,
            Message::MergedWarpMessage(MergedWarpMessage {
                msgs: vec![request(), request()],
                msg_ids: vec![7, 8],
            }),
        );
        frame.message_type = RequestType::RequestSync;
        peer::write(&mut socket, frame).await;
        let reply = peer::read(&mut socket).await;
        assert_eq!(reply.id, -112);
        let Some(Message::MergeResultMessage(result)) = reply.body else {
            panic!("batch reply missing")
        };
        assert_eq!(result.msgs.unwrap().len(), 2);
        tokio::task::yield_now().await;
    }
    assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
    {
        let events = captured.lock().unwrap();
        let writes: Vec<_> = events
            .iter()
            .filter(|e| {
                e.get("target")
                    .is_some_and(|v| v == "seata_rm::callback_write")
            })
            .collect();
        let decisions: Vec<_> = events
            .iter()
            .filter(|e| e.get("target").is_some_and(|v| v == "seata_rm::callback"))
            .collect();
        assert_eq!(writes.len(), 1);
        assert_eq!(decisions.len(), 2);
        assert_eq!(writes[0]["response_count"], "2");
        assert_eq!(writes[0]["outcome"], "local_written");
        for decision in decisions {
            assert_eq!(decision["request_id"], writes[0]["request_id"]);
            assert_eq!(
                decision["connection_generation"],
                writes[0]["connection_generation"]
            );
        }
        assert!(!format!("{writes:?}").contains("write-event-secret"));
    }
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
}

#[tokio::test(flavor = "current_thread")]
async fn closed_original_generation_emits_not_sent_without_using_replacement() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut resource = fixture::resource();
    Arc::get_mut(&mut resource).unwrap().pause = true;
    let catalog = ResourceCatalog::new(4, 4096).unwrap();
    catalog.register(resource.clone()).unwrap();
    let rm = ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog,
        fixture::journal().await,
    )
    .build()
    .unwrap();
    let mut socket = registered(&listener).await;
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(captured.clone()));
        let mut frame = peer::reply(-113, request());
        frame.message_type = RequestType::RequestSync;
        peer::write(&mut socket, frame).await;
        tokio::time::timeout(Duration::from_secs(2), resource.entered.notified())
            .await
            .unwrap();
        let old_generation = rm.connection_snapshot().slots[0]
            .generation
            .unwrap()
            .value();
        drop(socket);
        let mut replacement = tokio::time::timeout(Duration::from_secs(2), registered(&listener))
            .await
            .unwrap();
        resource.release.notify_one();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if captured.lock().unwrap().iter().any(|e| {
                    e.get("target")
                        .is_some_and(|v| v == "seata_rm::callback_write")
                }) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        {
            let events = captured.lock().unwrap();
            let writes: Vec<_> = events
                .iter()
                .filter(|e| {
                    e.get("target")
                        .is_some_and(|v| v == "seata_rm::callback_write")
                })
                .collect();
            assert_eq!(writes.len(), 1);
            assert_eq!(writes[0]["outcome"], "not_sent");
            assert_eq!(writes[0]["request_id"], "-113");
            assert_eq!(
                writes[0]["connection_generation"],
                old_generation.to_string()
            );
            assert_eq!(writes[0].len(), 6);
        }
        assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
        rm.shutdown().await.unwrap();
        peer::eof(&mut replacement).await;
    }
}
