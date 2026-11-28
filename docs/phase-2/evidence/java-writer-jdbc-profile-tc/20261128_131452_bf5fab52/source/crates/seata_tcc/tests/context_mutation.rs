//! 固定原 Java 的可变上下文、跟踪标志和精确原值保存。
#![allow(clippy::unwrap_used)]
use seata_tcc::{BusinessActionContext, TccError};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

fn raw(value: &str) -> Box<RawValue> {
    RawValue::from_string(value.into()).unwrap()
}
fn context() -> BusinessActionContext {
    BusinessActionContext::new(
        Some("original-xid".into()),
        9007199254740995,
        Some("original-action".into()),
        None,
    )
    .unwrap()
}

#[test]
fn pinned_java_value_change_and_type_comparisons_match_without_reencoding_values() {
    let observations: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/java_mutation.json")).unwrap();
    let mut count = 0;
    for row in observations.iter().filter(|r| r["kind"] == "pair") {
        let before = row["before"].as_str().unwrap();
        let after = row["after"].as_str().unwrap();
        let mut context = context();
        context.put_action_context("value", &raw(before)).unwrap();
        context.enable_action_context_tracking();
        let changed = context
            .add_action_context("value", Some(&raw(after)))
            .unwrap();
        assert_eq!(
            changed,
            row["changed"].as_bool().unwrap(),
            "{before} -> {after}"
        );
        assert_eq!(
            context.get_updated(),
            row.get("updated").and_then(serde_json::Value::as_bool)
        );
        assert_eq!(
            context.get_action_context("value").unwrap().get(),
            if after == "null" { before } else { after }
        );
        assert_eq!(context.xid(), Some("original-xid"));
        assert_eq!(context.branch_id(), 9007199254740995);
        count += 1;
    }
    assert_eq!(count, 30);
}

#[test]
fn pinned_java_tracking_null_removal_clear_replace_and_delay_lifecycle_match() {
    let observations: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/java_mutation.json")).unwrap();
    let mut context = context();
    let mut count = 0;
    for row in observations
        .iter()
        .filter(|r| r["kind"] == "state" && r["name"] != "util_delay_object")
    {
        let name = row["name"].as_str().unwrap();
        match name {
            "initial" => {}
            "untracked_put" => {
                context.put_action_context("a", &raw("1")).unwrap();
            }
            "enable" => context.enable_action_context_tracking(),
            "same_put" => {
                context.put_action_context("a", &raw("1")).unwrap();
            }
            "new_null" => {
                context.put_action_context("null", &raw("null")).unwrap();
            }
            "same_null" => {
                context.set_updated(None);
                context.put_action_context("null", &raw("null")).unwrap();
            }
            "missing_remove" => {
                assert!(context.remove_action_context("missing").is_none());
            }
            "remove_null" => {
                assert_eq!(context.remove_action_context("null").unwrap().get(), "null");
            }
            "clear" => {
                context.set_updated(Some(false));
                context.clear_action_context();
            }
            "clear_empty" => {
                context.set_updated(None);
                context.clear_action_context();
            }
            "replace_map" => context.set_action_context(BTreeMap::new()).unwrap(),
            "delay_true" => {
                context.set_delay_report(Some(true));
                context
                    .add_action_context("delayed", Some(&raw("2")))
                    .unwrap();
            }
            "delay_false" => context.set_delay_report(Some(false)),
            "reset" => {
                context.set_updated(None);
                context.set_delay_report(None);
            }
            "add_null" => {
                assert!(!context.add_action_context("ignored", None).unwrap());
            }
            _ => panic!("unexpected fixed Java state"),
        }
        assert_eq!(
            context.get_updated(),
            row.get("updated").and_then(serde_json::Value::as_bool),
            "{name}"
        );
        assert_eq!(
            context.get_delay_report(),
            row.get("delay_report").and_then(serde_json::Value::as_bool),
            "{name}"
        );
        // 原 Provider 为映射写入 @class 元数据并忽略 null；比较真实非空业务条目。
        let java = row["values"].as_object().unwrap();
        let actual: BTreeMap<String, serde_json::Value> = context
            .action_context()
            .iter()
            .filter(|(_, v)| v.get() != "null")
            .map(|(k, v)| (k.clone(), serde_json::from_str(v.get()).unwrap()))
            .collect();
        let expected: BTreeMap<String, serde_json::Value> = java
            .iter()
            .filter(|(k, _)| k.as_str() != "@class")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        assert_eq!(actual, expected, "{name}");
        count += 1;
    }
    assert_eq!(count, 15);
    context.set_updated(None);
    context
        .put_action_context("tracked_after_set", &raw("true"))
        .unwrap();
    assert_eq!(context.get_updated(), Some(true));
}

#[test]
fn failed_mutations_preserve_map_tracking_and_original_numeric_bytes() {
    let mut context = context();
    context.enable_action_context_tracking();
    let original = raw("1.23000000000000000000000000001");
    context.put_action_context("precise", &original).unwrap();
    context.set_updated(None);
    let before = context.to_application_data().unwrap();
    let huge = raw(&format!(
        "\"{}\"",
        "x".repeat(BusinessActionContext::MAX_CONTEXT_BYTES)
    ));
    assert_eq!(
        context.put_action_context("too_big", &huge).unwrap_err(),
        TccError::ContextBudget
    );
    assert_eq!(context.to_application_data().unwrap(), before);
    assert_eq!(context.get_updated(), None);
    let deep = raw(&("[".repeat(64) + "0" + &"]".repeat(64)));
    assert_eq!(
        context.put_action_context("deep", &deep).unwrap_err(),
        TccError::ContextBudget
    );
    assert_eq!(context.to_application_data().unwrap(), before);
    assert_eq!(context.get_updated(), None);
    let mut replacement = BTreeMap::new();
    replacement.insert("too_big".into(), huge);
    assert_eq!(
        context.set_action_context(replacement).unwrap_err(),
        TccError::ContextBudget
    );
    assert_eq!(context.to_application_data().unwrap(), before);
    assert_eq!(context.get_updated(), None);
    assert_eq!(
        context.get_action_context("precise").unwrap().get(),
        original.get()
    );
    let debug = format!("{context:?}");
    assert!(!debug.contains("original-xid"));
    assert!(!debug.contains("1.230000"));
}

#[test]
fn explicit_add_marks_updated_before_tracking_but_direct_put_does_not() {
    let mut context = context();
    context.put_action_context("framework", &raw("1")).unwrap();
    assert_eq!(context.get_updated(), None);
    assert!(context
        .add_action_context("business", Some(&raw("2")))
        .unwrap());
    assert_eq!(context.get_updated(), Some(true));
    context.set_updated(None);
    assert!(!context
        .add_action_context("business", Some(&raw("2")))
        .unwrap());
    assert_eq!(context.get_updated(), None);
    context.set_delay_report(Some(true));
    assert_eq!(context.get_updated(), None);
    assert_eq!(context.get_delay_report(), Some(true));
}
