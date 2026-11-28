//! 官方 Layer 组合保持应用过滤及 span 生命周期，Seata 只输出自身固定字段。
#![cfg(feature = "structured_log_layer")]
#![allow(clippy::unwrap_used)]
use seata_observability::{StructuredLogConfig, StructuredLogSubscriber};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tracing::{
    span::{Attributes, Id, Record},
    Event, Subscriber,
};
use tracing_subscriber::{
    filter::{dynamic_filter_fn, filter_fn},
    layer::{Context, SubscriberExt},
    registry::{LookupSpan, Registry},
    Layer,
};

#[derive(Default)]
struct AppState {
    span_fields: BTreeMap<u64, BTreeMap<String, String>>,
    parents: BTreeMap<u64, Option<u64>>,
    events: Vec<(String, Option<u64>, BTreeMap<String, String>)>,
    opened: Vec<u64>,
    closed: Vec<u64>,
    entered: usize,
    exited: usize,
    recorded: usize,
    follows: Vec<(u64, u64)>,
}
#[derive(Clone)]
struct AppLayer(Arc<Mutex<AppState>>);
impl<S> Layer<S> for AppLayer
where
    S: Subscriber,
    for<'a> S: LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = BTreeMap::new();
        attrs.record(
            &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
                fields.insert(field.name().into(), format!("{value:?}"));
            },
        );
        let parent = ctx.span(id).unwrap().parent().map(|s| s.id().into_u64());
        let mut state = self.0.lock().unwrap();
        state.opened.push(id.into_u64());
        state.parents.insert(id.into_u64(), parent);
        state.span_fields.insert(id.into_u64(), fields);
    }
    fn on_record(&self, id: &Id, record: &Record<'_>, _ctx: Context<'_, S>) {
        let mut state = self.0.lock().unwrap();
        state.recorded += 1;
        let fields = state.span_fields.get_mut(&id.into_u64()).unwrap();
        record.record(
            &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
                fields.insert(field.name().into(), format!("{value:?}"));
            },
        );
    }
    fn on_follows_from(&self, id: &Id, from: &Id, _ctx: Context<'_, S>) {
        self.0
            .lock()
            .unwrap()
            .follows
            .push((id.into_u64(), from.into_u64()));
    }
    fn on_enter(&self, _id: &Id, _ctx: Context<'_, S>) {
        self.0.lock().unwrap().entered += 1;
    }
    fn on_exit(&self, _id: &Id, _ctx: Context<'_, S>) {
        self.0.lock().unwrap().exited += 1;
    }
    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        self.0.lock().unwrap().closed.push(id.into_u64());
    }
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = BTreeMap::new();
        event.record(
            &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
                fields.insert(field.name().into(), format!("{value:?}"));
            },
        );
        let parent = ctx.event_span(event).map(|s| s.id().into_u64());
        self.0
            .lock()
            .unwrap()
            .events
            .push((event.metadata().target().into(), parent, fields));
    }
}
fn app_event() {
    tracing::info!(target:"app::composition",payload="application-private-payload","application event");
}
fn seata_event() {
    tracing::info!(target:"seata_tm::rpc",operation="Status",xid="tc:layer",request_id=-17,outcome="response","tm_rpc_outcome");
}
#[tokio::test]
async fn dynamic_application_filter_keeps_disabled_spans_and_independent_seata_events() {
    let baseline_state = Arc::new(Mutex::new(AppState::default()));
    let baseline = Registry::default().with(
        AppLayer(baseline_state.clone())
            .with_filter(dynamic_filter_fn(|_metadata, _context| false)),
    );
    let baseline_disabled = tracing::subscriber::with_default(baseline, || {
        tracing::info_span!(target:"app::composition","baseline-disabled-application",password="private-disabled-span").is_disabled()
    });
    assert!(baseline_state.lock().unwrap().opened.is_empty());
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(8, 4096).unwrap());
    let enabled = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(AppState::default()));
    let selected = enabled.clone();
    let stack = Registry::default()
        .with(
            AppLayer(state.clone()).with_filter(dynamic_filter_fn(move |meta, _context| {
                meta.target().starts_with("app::") && selected.load(Ordering::SeqCst)
            })),
        )
        .with(subscriber.into_layer());
    let disabled = tracing::subscriber::with_default(stack, || {
        let off = tracing::info_span!(target:"app::composition","disabled-application",password="private-disabled-span");
        assert_eq!(
            off.is_disabled(),
            baseline_disabled,
            "Seata layer changed original Registry admission"
        );
        app_event();
        seata_event();
        enabled.store(true, Ordering::SeqCst);
        app_event();
        enabled.store(false, Ordering::SeqCst);
        app_event();
        off.is_disabled()
    });
    assert_eq!(
        state.lock().unwrap().events.len(),
        1,
        "application event missing from sibling layer"
    );
    assert_eq!(state.lock().unwrap().opened.len(), 0);
    assert_eq!(sampler.snapshot().accepted, 1);
    let bytes = receiver.recv().await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["fields"]["request_id"], "-17");
    assert!(!String::from_utf8(bytes)
        .unwrap()
        .contains("application-private"));
    assert!(receiver.recv().await.is_none());
    let app = state.lock().unwrap();
    save(
        "dynamic-application.json",
        &serde_json::json!({"baseline_disabled_span":baseline_disabled,"disabled_span":disabled,"application_events":app.events,"opened":app.opened,"seata_event":value,"accepted":sampler.snapshot().accepted}),
    );
}
#[tokio::test]
async fn application_span_records_follows_clone_context_and_final_close_are_preserved() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(8, 4096).unwrap());
    let state = Arc::new(Mutex::new(AppState::default()));
    let stack = Registry::default()
        .with(AppLayer(state.clone()))
        .with(subscriber.into_layer());
    let (root_id, child_id) = tracing::subscriber::with_default(stack, || {
        let root = tracing::info_span!(target:"app::composition","application-root",password="private-span-password",progress=tracing::field::Empty);
        let root_id = root.id().unwrap().into_u64();
        let clone = root.clone();
        root.record("progress", 7_i64);
        let child = {
            let _entered = root.enter();
            let child = tracing::info_span!(target:"app::composition","application-child");
            let _child_entered = child.enter();
            assert_eq!(tracing::Span::current().id(), child.id());
            app_event();
            seata_event();
            drop(_child_entered);
            child
        };
        let child_id = child.id().unwrap().into_u64();
        child.follows_from(root.id());
        drop(child);
        drop(root);
        assert!(
            !state.lock().unwrap().closed.contains(&root_id),
            "span closed before last clone"
        );
        drop(clone);
        (root_id, child_id)
    });
    {
        let state = state.lock().unwrap();
        assert_eq!(state.parents[&child_id], Some(root_id));
        assert_eq!(
            state.span_fields[&root_id]["password"],
            "\"private-span-password\""
        );
        assert_eq!(state.span_fields[&root_id]["progress"], "7");
        assert_eq!(state.opened.len(), 2);
        assert!(state.closed.contains(&root_id) && state.closed.contains(&child_id));
        assert_eq!(state.closed.len(), 2);
        assert_eq!(state.recorded, 1);
        assert_eq!(state.entered, state.exited);
        assert!(state.entered >= 2);
        assert_eq!(state.follows, vec![(child_id, root_id)]);
        assert_eq!(state.events.len(), 2);
        assert!(state.events.iter().all(|e| e.1 == Some(child_id)));
        save(
            "application-lifecycle.json",
            &serde_json::json!({"root_id":root_id.to_string(),"child_id":child_id.to_string(),"parents":state.parents,"span_fields":state.span_fields,"events":state.events,"opened":state.opened,"closed":state.closed,"entered":state.entered,"exited":state.exited,"recorded":state.recorded,"follows":state.follows}),
        );
    }
    let bytes = receiver.recv().await.unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        !text.contains("private-span-password") && !text.contains("application-private-payload")
    );
    assert_eq!(sampler.snapshot().accepted, 1);
    assert!(receiver.recv().await.is_none());
}

fn save(name: &str, value: &serde_json::Value) {
    if let Some(folder) = std::env::var_os("SEATA_STRUCTURED_LOG_OUT") {
        let folder = std::path::PathBuf::from(folder);
        assert!(
            folder.is_absolute()
                && (folder.to_string_lossy().starts_with("D:")
                    || folder.to_string_lossy().starts_with("E:"))
        );
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }
}

#[tokio::test]
async fn all_seven_templates_and_application_output_keep_separate_admission() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(16, 4096).unwrap());
    let state = Arc::new(Mutex::new(AppState::default()));
    let digest = "a".repeat(64);
    let stack = Registry::default()
        .with(AppLayer(state.clone()).with_filter(filter_fn(|m| m.target().starts_with("app::"))))
        .with(subscriber.into_layer());
    tracing::subscriber::with_default(stack, || {
        app_event();
        seata_event();
        tracing::warn!(target:"seata_rm::rpc",operation="BranchReport",xid=?Some("tc:layer"),branch_id=?Some(42_i64),resource_sha256=?Some(digest.as_str()),request_id=?Some(-18_i32),outcome="unknown","rm_rpc_outcome");
        tracing::info!(target:"seata_rm::callback",request_id=-18,connection_generation=2_u64,xid="tc:layer",branch_id=42_i64,resource_sha256=digest.as_str(),phase="Rollback",branch_status="PhaseTwoRollbacked",result_code="Success","rm_callback_decision");
        tracing::info!(target:"seata_rm::callback_write",request_id=-18,connection_generation=2_u64,response_count=1_u64,outcome="local_written","rm_callback_response_write");
        tracing::info!(target:"seata_db_mysql::control",operation="rollback",requested_phase="rollback",xid="tc:layer",branch_id=42_i64,resource_sha256=digest.as_str(),outcome="unknown_commit",resolution="not_observed",commit_attempted=true,"mysql_control_outcome");
        tracing::info!(target:"seata_db_mysql::undo",operation="rollback",xid="tc:layer",branch_id=42_i64,resource_sha256=digest.as_str(),outcome="completed",observation="consumed",commit_attempted=true,"mysql_undo_outcome");
        tracing::warn!(target:"seata_db_mysql::undo",xid="tc:layer",branch_id=42_i64,resource_digest=digest.as_str(),outcome="unknown","mysql_undo_checksum_rejected");
    });
    let mut records = Vec::new();
    while let Some(bytes) = receiver.recv().await {
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("application-private-payload"));
        records.push(serde_json::from_str::<serde_json::Value>(&text).unwrap());
    }
    assert_eq!(records.len(), 7, "composed layer lost Seata events");
    assert_eq!(
        state.lock().unwrap().events.len(),
        1,
        "application event missing from sibling layer"
    );
    assert_eq!(sampler.snapshot().accepted, 7);
    assert_eq!(sampler.snapshot().dropped_invalid, 0);
    save(
        "seven-composed-templates.json",
        &serde_json::json!({"records":records,"application_event_count":state.lock().unwrap().events.len(),"accepted":sampler.snapshot().accepted}),
    );
}
#[tokio::test]
async fn application_global_filter_still_controls_seata_events() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(4, 4096).unwrap());
    let stack = Registry::default()
        .with(subscriber.into_layer())
        .with(tracing_subscriber::filter::LevelFilter::WARN);
    tracing::subscriber::with_default(stack, || {
        seata_event();
        tracing::warn!(target:"seata_tm::rpc",operation="Status",xid="tc:layer",request_id=-18,outcome="unknown","tm_rpc_outcome");
    });
    let bytes = receiver.recv().await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["level"], "WARN");
    assert_eq!(value["fields"]["request_id"], "-18");
    assert!(receiver.recv().await.is_none());
    assert_eq!(sampler.snapshot().accepted, 1);
    save("global-filter.json", &value);
}
#[path = "../../seata_tm/tests/support/tm_peer.rs"]
mod fixture;
#[tokio::test]
async fn actual_tm_and_instrumented_application_span_share_stack_without_json_enrichment() {
    use seata_protocol::{transaction::GlobalStatusResponse, GlobalStatus, Message};
    use tracing::Instrument;
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(1, 4096).unwrap());
    let state = Arc::new(Mutex::new(AppState::default()));
    let stack = Registry::default()
        .with(AppLayer(state.clone()).with_filter(filter_fn(|m| m.target().starts_with("app::"))))
        .with(subscriber.into_layer());
    let _guard = tracing::subscriber::set_default(stack);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = fixture::manager(
        listener.local_addr().unwrap(),
        std::time::Duration::from_secs(2),
    );
    let server = tokio::spawn(async move {
        let mut socket = fixture::registered(&listener).await;
        let mut ids = Vec::new();
        for _ in 0..2 {
            let request = fixture::peer::read(&mut socket).await;
            assert!(matches!(
                request.body,
                Some(Message::GlobalStatusRequest(_))
            ));
            ids.push(request.id);
            fixture::peer::write(
                &mut socket,
                fixture::peer::reply(
                    request.id,
                    Message::GlobalStatusResponse(GlobalStatusResponse {
                        response: fixture::peer::success(),
                        global_status: GlobalStatus::Rollbacking,
                    }),
                ),
            )
            .await;
        }
        fixture::peer::eof(&mut socket).await;
        ids
    });
    let root = tracing::info_span!(target:"app::composition","application-tm-span",password="private-tm-span");
    let root_id = root.id().unwrap().into_u64();
    let xid = seata_tm::Xid::parse("tc:layer-native".into()).unwrap();
    async {
        app_event();
        for _ in 0..2 {
            assert_eq!(
                tm.get_status(&xid).await.unwrap().status(),
                GlobalStatus::Rollbacking
            );
        }
    }
    .instrument(root.clone())
    .await;
    drop(root);
    tm.shutdown().await.unwrap();
    let ids = server.await.unwrap();
    let bytes = receiver.recv().await.unwrap();
    receiver.close();
    let event: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(event["fields"]["request_id"], ids[0].to_string());
    assert_eq!(event["fields"].as_object().unwrap().len(), 5);
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("private-tm-span") && !text.contains("application-private-payload"));
    assert_eq!(
        (
            sampler.snapshot().accepted,
            sampler.snapshot().received,
            sampler.snapshot().dropped_full
        ),
        (1, 1, 1)
    );
    {
        let state = state.lock().unwrap();
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.events[0].1, Some(root_id));
        assert_eq!(state.closed, vec![root_id]);
        assert_eq!(state.entered, state.exited);
    }
    let app_proof = {
        let app = state.lock().unwrap();
        serde_json::json!({"opened":app.opened,"closed":app.closed,"events":app.events,"entered":app.entered,"exited":app.exited})
    };
    save(
        "actual-tm-composition.json",
        &serde_json::json!({"actual_request_ids":ids,"event":event,"application_span_id":root_id.to_string(),"application_state":app_proof,"application_closed_once":true,"application_events":1,"accepted":sampler.snapshot().accepted,"received":sampler.snapshot().received,"dropped_full":sampler.snapshot().dropped_full}),
    );
}
