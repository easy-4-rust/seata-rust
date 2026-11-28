//! 原上下文精度、输入预算与完整三阶段 Fence 决策的可观察行为。
#![allow(clippy::unwrap_used)]
use seata_tcc::{
    BusinessActionContext, CommonFenceStatus, FenceDecision, FenceHandler, FenceOperation, TccError,
};

#[test]
fn original_context_values_and_identity_are_preserved_without_floating_point() {
    let raw = r#"{"actionContext":{"actionName":"cannot-override","useTCCFence":true,"large":1234567890123456789012345678901234567890,"decimal":1.23000000000000000000000000001,"exponent":1e10000,"branchId":"spoofed","nested":{"x":[null,true,"中文"]}}}"#;
    let context = BusinessActionContext::new(
        Some("original-xid".into()),
        i64::MAX,
        Some("original-action".into()),
        Some(raw),
    )
    .unwrap();
    assert_eq!(context.xid(), Some("original-xid"));
    assert_eq!(context.branch_id(), i64::MAX);
    assert_eq!(context.action_name(), Some("original-action"));
    assert!(context.use_common_fence());
    for (key, value) in [
        ("large", "1234567890123456789012345678901234567890"),
        ("decimal", "1.23000000000000000000000000001"),
        ("exponent", "1e10000"),
    ] {
        assert_eq!(context.get_action_context(key).unwrap().get(), value);
    }
    let encoded = context.to_application_data().unwrap();
    let roundtrip = BusinessActionContext::new(None, -1, None, Some(&encoded)).unwrap();
    for (key, value) in context.action_context() {
        assert_eq!(
            roundtrip.get_action_context(key).unwrap().get(),
            value.get()
        );
    }
    let diagnostic = format!("{context:?}");
    for secret in [
        "original-xid",
        "original-action",
        "cannot-override",
        "spoofed",
        "123456789",
    ] {
        assert!(!diagnostic.contains(secret));
    }
}

#[test]
fn java_blank_null_missing_and_boolean_flag_semantics_are_explicit() {
    for data in [
        None,
        Some(""),
        Some(" \t\r\n\u{001c}\u{2003}"),
        Some("{}"),
        Some("{\"actionContext\":null}"),
    ] {
        let context = BusinessActionContext::new(None, i64::MIN, None, data).unwrap();
        assert!(context.action_context().is_empty());
        assert!(!context.use_common_fence());
        assert_eq!(context.branch_id(), i64::MIN);
        assert_eq!(context.xid(), None);
        assert_eq!(context.action_name(), None);
        assert_eq!(
            context.to_application_data().unwrap(),
            "{\"actionContext\":{}}"
        );
    }
    for value in ["false", "null", "1", "\"true\"", "[]", "{}"] {
        let data = format!("{{\"actionContext\":{{\"useTCCFence\":{value}}}}}");
        assert!(!BusinessActionContext::new(None, 0, None, Some(&data))
            .unwrap()
            .use_common_fence());
    }
    let data = r#"{"actionContext":{"key":1,"key":2},"actionContext":{"last":true}}"#;
    let context = BusinessActionContext::new(None, 0, None, Some(data)).unwrap();
    assert!(context.get_action_context("key").is_none());
    assert_eq!(context.get_action_context("last").unwrap().get(), "true");
}

#[test]
fn invalid_context_and_budgets_fail_before_context_can_be_used() {
    for data in [
        "null",
        "[]",
        "true",
        "1",
        "{",
        "{\"actionContext\":[]}",
        "{\"actionContext\":1}",
        "{\"actionContext\":\"bad\"}",
        "{\"actionContext\":{\"n\":01}}",
        "{\"actionContext\":{\"n\":NaN}}",
        "{} trailing",
        "\u{00a0}",
        "\u{0085}",
        "\u{2007}",
        "\u{202f}",
    ] {
        let error = BusinessActionContext::new(None, 0, None, Some(data)).unwrap_err();
        assert!(
            matches!(error, TccError::InvalidContext | TccError::ContextNotObject),
            "accepted {data}"
        );
    }
    let oversized = " ".repeat(1_048_577);
    let boundary = " ".repeat(1_048_576);
    assert!(BusinessActionContext::new(None, 0, None, Some(&boundary)).is_ok());
    assert_eq!(
        BusinessActionContext::new(None, 0, None, Some(&oversized)).unwrap_err(),
        TccError::ContextBudget
    );
    let deep = format!(
        "{{\"actionContext\":{{\"v\":{}0{}}}}}",
        "[".repeat(63),
        "]".repeat(63)
    );
    assert_eq!(
        BusinessActionContext::new(None, 0, None, Some(&deep)).unwrap_err(),
        TccError::ContextBudget
    );
    let bounded = format!(
        "{{\"actionContext\":{{\"v\":{}0{}}}}}",
        "[".repeat(62),
        "]".repeat(62)
    );
    assert!(BusinessActionContext::new(None, 0, None, Some(&bounded)).is_ok());
    let braces = r#"{"actionContext":{"v":"[{{[\\\""}}"#;
    assert!(BusinessActionContext::new(None, 0, None, Some(braces)).is_ok());
}

#[test]
fn complete_fence_matrix_never_replays_try_or_changes_terminal_direction() {
    use CommonFenceStatus::{Committed, Rollbacked, Suspended, Tried};
    use FenceDecision::{
        AlreadyApplied, InsertAndInvoke, InsertSuspended, MissingTry, Refuse, UpdateAndInvoke,
    };
    use FenceOperation::{Cancel, Confirm, Try};
    let states = [
        None,
        Some(Tried),
        Some(Committed),
        Some(Rollbacked),
        Some(Suspended),
    ];
    let expected = [
        (
            Try,
            [InsertAndInvoke(Tried), Refuse, Refuse, Refuse, Refuse],
        ),
        (
            Confirm,
            [
                MissingTry,
                UpdateAndInvoke {
                    expected: Tried,
                    next: Committed,
                },
                AlreadyApplied,
                Refuse,
                Refuse,
            ],
        ),
        (
            Cancel,
            [
                InsertSuspended,
                UpdateAndInvoke {
                    expected: Tried,
                    next: Rollbacked,
                },
                Refuse,
                AlreadyApplied,
                AlreadyApplied,
            ],
        ),
    ];
    for (operation, decisions) in expected {
        for (state, decision) in states.into_iter().zip(decisions) {
            assert_eq!(FenceHandler::plan(operation, state), decision);
        }
    }
    for status in [Tried, Committed, Rollbacked, Suspended] {
        assert_eq!(CommonFenceStatus::try_from(status.code()).unwrap(), status);
    }
    for code in [i32::MIN, -1, 0, 5, 255, i32::MAX] {
        assert_eq!(
            CommonFenceStatus::try_from(code),
            Err(TccError::UnknownFenceStatus)
        );
    }
}
