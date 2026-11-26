//! 既有78个实际Java处理器输出的正式API字段验证；未执行/模拟业务处理器。
#![allow(clippy::unwrap_used)]
use seata_protocol::transaction::AbstractTransactionResponse;
use seata_protocol::{Message, PayloadCodec};
use std::collections::BTreeSet;

fn names() -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for op in [
        "begin",
        "commit",
        "rollback",
        "status",
        "report",
        "register",
        "branch_report",
        "lock",
    ] {
        for failure in ["none", "transaction", "runtime", "store"] {
            result.insert(format!("matrix_{op}_{failure}"));
        }
    }
    for op in ["commit", "rollback", "status"] {
        for failure in ["transaction", "runtime", "store"] {
            for lookup in ["absent", "throws", "getter_throws", "mutated_then_throws"] {
                result.insert(format!("fallback_{op}_{failure}_{lookup}"));
            }
        }
    }
    for name in [
        "conflict_lockkeyconflict",
        "conflict_lockkeyconflictfailfast",
        "lock_success_false",
        "auth_rm_deny_closed",
    ] {
        result.insert(name.into());
    }
    for role in ["tm", "rm"] {
        for auth in ["allow", "deny", "throw"] {
            result.insert(format!("auth_{role}_{auth}"));
        }
    }
    assert_eq!(result.len(), 78);
    result
}
fn hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn transaction(
    op: &str,
    r: &AbstractTransactionResponse,
    status: i32,
    xid: Option<&str>,
    branch_id: i64,
    lockable: bool,
) -> String {
    format!(
        "{op}\t{}\t{}\t{}\t{status}\t{}\t{branch_id}\t{}\t",
        r.result_code.to_java_v1(),
        r.transaction_exception_code.to_java_v1(),
        hex(r.msg.as_deref().unwrap_or_default()),
        hex(xid.unwrap_or_default()),
        u8::from(lockable)
    )
}
fn normalized(message: &Message) -> String {
    match message {
        Message::GlobalBeginResponse(v) => {
            assert!(v.extra_data.is_none());
            transaction("BEGIN", &v.response, -1, v.xid.as_deref(), 0, false)
        }
        Message::GlobalCommitResponse(v) => transaction(
            "COMMIT",
            &v.response,
            i32::from(v.global_status.to_java_v1()),
            None,
            0,
            false,
        ),
        Message::GlobalRollbackResponse(v) => transaction(
            "ROLLBACK",
            &v.response,
            i32::from(v.global_status.to_java_v1()),
            None,
            0,
            false,
        ),
        Message::GlobalStatusResponse(v) => transaction(
            "STATUS",
            &v.response,
            i32::from(v.global_status.to_java_v1()),
            None,
            0,
            false,
        ),
        Message::GlobalReportResponse(v) => transaction(
            "REPORT",
            &v.response,
            i32::from(v.global_status.to_java_v1()),
            None,
            0,
            false,
        ),
        Message::BranchRegisterResponse(v) => {
            transaction("REGISTER", &v.response, -1, None, v.branch_id, false)
        }
        Message::BranchReportResponse(v) => {
            transaction("BRANCH_REPORT", &v.response, -1, None, 0, false)
        }
        Message::GlobalLockQueryResponse(v) => {
            transaction("LOCK", &v.response, -1, None, 0, v.lockable)
        }
        Message::RegisterTMResponse(v) => format!(
            "REG_TM\t{}\t0\t\t-1\t\t0\t0\t{}",
            u8::from(v.identified),
            hex(v.wire_version.as_deref().unwrap_or_default())
        ),
        Message::RegisterRMResponse(v) => format!(
            "REG_RM\t{}\t0\t\t-1\t\t0\t0\t{}",
            u8::from(v.identified),
            hex(v.wire_version.as_deref().unwrap_or_default())
        ),
        _ => panic!("unexpected handler result type"),
    }
}
#[test]
fn all_78_named_java_handler_contexts_match_independent_expectations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/protocol/v1/handler_matrix");
    let expected = names();
    let actual: BTreeSet<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|v| v == "bin"))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_string())
        .collect();
    assert_eq!(actual, expected);
    let codec = PayloadCodec::new(64 * 1024).unwrap();
    for name in expected {
        let bytes = std::fs::read(root.join(format!("{name}.bin"))).unwrap();
        let sidecar = std::fs::read_to_string(root.join(format!("{name}.expect"))).unwrap();
        let value = codec.decode(&bytes).unwrap();
        assert_eq!(normalized(&value), sidecar.trim_end_matches('\n'), "{name}");
        assert_eq!(codec.encode(&value).unwrap(), bytes, "{name}");
    }
}
