//! 固定原 Java util 的自动报告门槛、批量参数与失败状态，不把出口探针当成网络验收。
#![allow(clippy::unwrap_used)]
use seata_datasource::FieldValue;
use seata_tcc::{BusinessActionContext, BusinessActionContextUtil, TccError, TccTransaction};
use serde_json::{value::RawValue, Value};
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

/// 记录真实 util 调用到事务能力的载荷，模拟失败/取消而不签发网络成功证明。
/// 对应 Java: UtilOracle 的 ResourceManager 记录出口。
#[derive(Debug, Default)]
struct ReportProbe {
    reports: Vec<String>,
    reject: bool,
    pending: bool,
    closed: bool,
    no_report: bool,
}
impl TccTransaction for ReportProbe {
    fn report_context<'a>(
        &'a mut self,
        context: &'a mut BusinessActionContext,
    ) -> Pin<Box<dyn Future<Output = Result<bool, TccError>> + Send + 'a>> {
        Box::pin(async move {
            self.closed = true;
            self.reports.push(context.to_application_data()?);
            if self.pending {
                std::future::pending::<()>().await;
            }
            if self.reject {
                return Err(TccError::ContextReportUnavailable);
            }
            self.closed = false;
            Ok(!self.no_report)
        })
    }
    fn execute<'a>(
        &'a mut self,
        _sql: &'a str,
        _parameters: &'a [FieldValue],
    ) -> Pin<Box<dyn Future<Output = Result<u64, TccError>> + Send + 'a>> {
        Box::pin(async { Err(TccError::TransactionUnavailable) })
    }
}
fn ready<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::noop();
    match std::pin::pin!(future)
        .as_mut()
        .poll(&mut Context::from_waker(waker))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("expected immediately resolved util probe"),
    }
}
fn raw(s: &str) -> Box<RawValue> {
    RawValue::from_string(s.into()).unwrap()
}
fn context(before: &str) -> BusinessActionContext {
    BusinessActionContext::new(
        Some("original-xid".into()),
        9007199254740995,
        Some("original-action".into()),
        Some(&format!("{{\"actionContext\":{before}}}")),
    )
    .unwrap()
}
fn values(context: &BusinessActionContext) -> Value {
    serde_json::from_str::<Value>(&context.to_application_data().unwrap()).unwrap()["actionContext"]
        .clone()
}
fn java_values(value: &Value) -> Value {
    let mut value = value.clone();
    value.as_object_mut().unwrap().remove("@class");
    value
}

#[test]
fn original_java_util_nineteen_state_and_report_observations_match() {
    let rows: Vec<Value> = serde_json::from_str(include_str!("fixtures/java_util.json")).unwrap();
    assert_eq!(rows.len(), 19);
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let mut ctx = context(row["before"].as_str().unwrap());
        ctx.enable_action_context_tracking();
        ctx.set_updated(row["initial_updated"].as_bool());
        ctx.set_delay_report(row["delay"].as_bool());
        let mut probe = ReportProbe {
            reject: row["reject"].as_bool().unwrap(),
            ..ReportProbe::default()
        };
        let additions = row["additions"].as_str();
        let result = if row["report_only"] == true {
            ready(BusinessActionContextUtil::report_context(
                &mut ctx, &mut probe,
            ))
        } else if row["batch"] == true {
            let batch = additions
                .map(|s| serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(s).unwrap());
            ready(BusinessActionContextUtil::add_context_map(
                &mut ctx,
                &mut probe,
                batch.as_ref(),
            ))
        } else {
            let value = additions.map(raw);
            ready(BusinessActionContextUtil::add_context(
                &mut ctx,
                &mut probe,
                "value",
                value.as_deref(),
            ))
        };
        assert_eq!(result.is_err(), row["failed"].as_bool().unwrap(), "{name}");
        if let Ok(result) = result {
            assert_eq!(result, row["result"].as_bool().unwrap(), "{name}");
        }
        assert_eq!(ctx.get_updated(), row["updated"].as_bool(), "{name}");
        let mut expected = java_values(&row["values"]);
        if name == "same_double" {
            // Java Double 重编码为 1.0；兼容相等/报告行为，但 Rust 仍须保存新原值 1.00。
            assert_eq!(ctx.get_action_context("value").unwrap().get(), "1.00");
            expected["value"] = serde_json::from_str("1.00").unwrap();
        }
        assert_eq!(values(&ctx), expected, "{name}");
        let reports = row["reports"][1].as_array().unwrap();
        assert_eq!(probe.reports.len(), reports.len(), "{name}");
        for (ours, theirs) in probe.reports.iter().zip(reports) {
            assert_eq!(theirs["type"], "TCC");
            assert_eq!(theirs["status"], "Registered");
            assert_eq!(theirs["xid"], "original-xid");
            assert_eq!(theirs["branch_id"], "9007199254740995");
            let original: Value =
                serde_json::from_str(theirs["application_data"].as_str().unwrap()).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(ours).unwrap()["actionContext"],
                java_values(&original["actionContext"]),
                "{name}"
            );
        }
        assert_eq!(ctx.xid(), Some("original-xid"));
        assert_eq!(ctx.branch_id(), 9007199254740995);
    }
}

#[test]
fn original_java_util_delay_object_followup_is_now_compared() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/java_mutation.json")).unwrap();
    let row = rows
        .iter()
        .find(|r| r["name"] == "util_delay_object")
        .unwrap();
    let mut ctx = context(r#"{"delayed":2}"#);
    ctx.enable_action_context_tracking();
    ctx.set_delay_report(Some(true));
    let mut probe = ReportProbe::default();
    assert!(!ready(BusinessActionContextUtil::add_context(
        &mut ctx,
        &mut probe,
        "object",
        Some(&raw(r#"{"a":1}"#))
    ))
    .unwrap());
    assert_eq!(ctx.get_updated(), row["updated"].as_bool());
    assert_eq!(values(&ctx), java_values(&row["values"]));
    assert!(probe.reports.is_empty());
}

#[test]
fn batch_budget_rejection_is_atomic_and_preserves_raw_composite_precision() {
    let mut ctx = context(r#"{"keep":7}"#);
    ctx.set_updated(Some(false));
    let mut probe = ReportProbe::default();
    let too_big =
        serde_json::to_string(&"x".repeat(BusinessActionContext::MAX_CONTEXT_BYTES)).unwrap();
    let additions = BTreeMap::from([("a".into(), raw("1")), ("z".into(), raw(&too_big))]);
    assert_eq!(
        ready(BusinessActionContextUtil::add_context_map(
            &mut ctx,
            &mut probe,
            Some(&additions)
        )),
        Err(TccError::ContextBudget)
    );
    assert_eq!(values(&ctx), serde_json::json!({"keep":7}));
    assert_eq!(ctx.get_updated(), Some(false));
    assert!(probe.reports.is_empty());
    ctx.set_delay_report(Some(true));
    let original = r#"{ "n":1.23000000000000000000000000001,"a":[1,"x y"] }"#;
    assert!(!ready(BusinessActionContextUtil::add_context(
        &mut ctx,
        &mut probe,
        "object",
        Some(&raw(original))
    ))
    .unwrap());
    assert_eq!(
        serde_json::from_str::<String>(ctx.get_action_context("object").unwrap().get()).unwrap(),
        original
    );
    assert_eq!(ctx.get_updated(), Some(true));
    assert!(probe.reports.is_empty());
}

#[test]
fn only_confirmed_report_clears_updated_and_borrowed_contexts_remain_isolated() {
    let mut first = context("{}");
    let mut second = context("{}");
    let mut probe = ReportProbe::default();
    first.set_updated(Some(true));
    probe.no_report = true;
    assert!(!ready(BusinessActionContextUtil::report_context(
        &mut first, &mut probe
    ))
    .unwrap());
    assert_eq!(first.get_updated(), Some(true));
    probe.no_report = false;
    assert!(ready(BusinessActionContextUtil::add_context(
        &mut second,
        &mut probe,
        "value",
        Some(&raw("2"))
    ))
    .unwrap());
    assert_eq!(second.get_updated(), None);
    assert_eq!(values(&first), serde_json::json!({}));
    assert_eq!(first.get_updated(), Some(true));
}

#[test]
fn cancelled_auto_report_retains_dirty_context_and_does_not_reopen_transaction() {
    let mut ctx = context("{}");
    let mut probe = ReportProbe {
        pending: true,
        ..ReportProbe::default()
    };
    let value = raw("9");
    let waker = Waker::noop();
    {
        let mut future = std::pin::pin!(BusinessActionContextUtil::add_context(
            &mut ctx,
            &mut probe,
            "value",
            Some(&value)
        ));
        assert!(future
            .as_mut()
            .poll(&mut Context::from_waker(waker))
            .is_pending());
    }
    assert_eq!(ctx.get_updated(), Some(true));
    assert!(probe.closed);
    assert_eq!(probe.reports.len(), 1);
}
