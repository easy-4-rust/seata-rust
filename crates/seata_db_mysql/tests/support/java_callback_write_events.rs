//! 原 Java TC 现场额外捕获本地决策和写入事件，不替代原帧/持久化断言。
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
};

pub(super) async fn run(future: impl Future<Output = ()>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let _guard =
        tracing::subscriber::set_default(super::java_checksum_events::EventCapture(events.clone()));
    future.await;
    let events = events.lock().unwrap();
    let selected: Vec<&BTreeMap<String, String>> = events
        .iter()
        .filter(|event| {
            event.get("target").is_some_and(|target| {
                target == "seata_rm::callback" || target == "seata_rm::callback_write"
            })
        })
        .collect();
    let decisions: Vec<_> = selected
        .iter()
        .filter(|event| event["target"] == "seata_rm::callback")
        .collect();
    let writes: Vec<_> = selected
        .iter()
        .filter(|event| event["target"] == "seata_rm::callback_write")
        .collect();
    assert!(
        !decisions.is_empty(),
        "original TC decisions must be observed"
    );
    assert_eq!(
        writes.len(),
        decisions.len(),
        "selected original TC profiles use one branch per envelope"
    );
    for write in writes {
        assert_eq!(write.len(), 6);
        assert_eq!(write["message"], "rm_callback_response_write");
        assert_eq!(write["outcome"], "local_written");
        assert_eq!(write["response_count"], "1");
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| decision["request_id"] == write["request_id"]
                    && decision["connection_generation"] == write["connection_generation"])
                .count(),
            1
        );
    }
    let folder = std::path::PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    std::fs::write(
        folder.join("rm-callback-write-events.json"),
        serde_json::to_vec_pretty(&selected).unwrap(),
    )
    .unwrap();
}
