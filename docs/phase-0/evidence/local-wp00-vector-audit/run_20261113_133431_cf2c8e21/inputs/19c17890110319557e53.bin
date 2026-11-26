// Copyright 2026 Seata-Rust contributors.
// Licensed under the Apache License, Version 2.0.

use super::abstract_transaction_response::AbstractTransactionResponse;
use super::payload_message::PayloadMessage;
use std::collections::HashSet;
use std::path::PathBuf;

fn root() -> PathBuf {
    std::env::var_os("SEATA_IT_HANDLER_WIRE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/protocol/v1/handler_matrix")
        })
}

fn names() -> HashSet<String> {
    let mut names = HashSet::new();
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
            names.insert(format!("matrix_{op}_{failure}"));
        }
    }
    for op in ["commit", "rollback", "status"] {
        for failure in ["transaction", "runtime", "store"] {
            for lookup in ["absent", "throws", "getter_throws", "mutated_then_throws"] {
                names.insert(format!("fallback_{op}_{failure}_{lookup}"));
            }
        }
    }
    for name in [
        "conflict_lockkeyconflict",
        "conflict_lockkeyconflictfailfast",
        "lock_success_false",
    ] {
        names.insert(name.to_owned());
    }
    for role in ["tm", "rm"] {
        for auth in ["allow", "deny", "throw"] {
            names.insert(format!("auth_{role}_{auth}"));
        }
    }
    names.insert("auth_rm_deny_closed".to_owned());
    assert_eq!(names.len(), 78);
    names
}

fn hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn transaction(
    op: &str,
    response: &AbstractTransactionResponse,
    status: i32,
    xid: Option<&str>,
    branch_id: i64,
    lockable: bool,
) -> String {
    format!(
        "{op}\t{}\t{}\t{}\t{status}\t{}\t{branch_id}\t{}\t",
        response.result_code as u8,
        response.transaction_exception_code as u8,
        hex(response.msg.as_deref().unwrap_or_default()),
        hex(xid.unwrap_or_default()),
        u8::from(lockable)
    )
}

fn normalized(value: &PayloadMessage) -> String {
    match value {
        PayloadMessage::GlobalBeginResponse(v) => {
            transaction("BEGIN", &v.response, -1, v.xid.as_deref(), 0, false)
        }
        PayloadMessage::GlobalCommitResponse(v) => transaction(
            "COMMIT",
            &v.response,
            i32::from(v.global_status as u8),
            None,
            0,
            false,
        ),
        PayloadMessage::GlobalRollbackResponse(v) => transaction(
            "ROLLBACK",
            &v.response,
            i32::from(v.global_status as u8),
            None,
            0,
            false,
        ),
        PayloadMessage::GlobalStatusResponse(v) => transaction(
            "STATUS",
            &v.response,
            i32::from(v.global_status as u8),
            None,
            0,
            false,
        ),
        PayloadMessage::GlobalReportResponse(v) => transaction(
            "REPORT",
            &v.response,
            i32::from(v.global_status as u8),
            None,
            0,
            false,
        ),
        PayloadMessage::BranchRegisterResponse(v) => {
            transaction("REGISTER", &v.response, -1, None, v.branch_id, false)
        }
        PayloadMessage::BranchReportResponse(v) => {
            transaction("BRANCH_REPORT", &v.response, -1, None, 0, false)
        }
        PayloadMessage::GlobalLockQueryResponse(v) => {
            transaction("LOCK", &v.response, -1, None, 0, v.lockable)
        }
        PayloadMessage::RegisterTMResponse(v) => format!(
            "REG_TM\t{}\t0\t\t-1\t\t0\t0\t{}",
            u8::from(v.identified),
            hex(v.wire_version.as_deref().unwrap_or_default())
        ),
        PayloadMessage::RegisterRMResponse(v) => format!(
            "REG_RM\t{}\t0\t\t-1\t\t0\t0\t{}",
            u8::from(v.identified),
            hex(v.wire_version.as_deref().unwrap_or_default())
        ),
        other => panic!("Unexpected Java handler response type: {other:?}"),
    }
}

#[test]
fn every_runtime_handler_response_matches_independent_fields_and_bytes() {
    let expected_names = names();
    for extension in ["expect", "bin"] {
        let actual: HashSet<_> = std::fs::read_dir(root())
            .expect("Java runtime capture fixtures are required")
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == extension))
            .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            actual, expected_names,
            "all78 cases must execute; no unknown/fabricated null report"
        );
    }
    for name in expected_names {
        let bytes = std::fs::read(root().join(format!("{name}.bin"))).unwrap();
        let expected = std::fs::read_to_string(root().join(format!("{name}.expect"))).unwrap();
        let value = PayloadMessage::decode(&bytes).unwrap();
        assert_eq!(
            normalized(&value),
            expected.trim_end_matches(['\r', '\n']),
            "independent runtime fields: {name}"
        );
        assert_eq!(
            value.encode().unwrap(),
            bytes,
            "exact actual Java runtime payload: {name}"
        );
    }
}

#[test]
fn every_runtime_handler_response_rejects_truncation_and_trailing_bytes() {
    for name in names() {
        let mut bytes = std::fs::read(root().join(format!("{name}.bin")))
            .expect("Java runtime capture fixtures are required");
        for end in 0..bytes.len() {
            assert!(
                PayloadMessage::decode(&bytes[..end]).is_err(),
                "{name} prefix {end}"
            );
        }
        bytes.push(0);
        assert!(
            PayloadMessage::decode(&bytes).is_err(),
            "{name} trailing data"
        );
    }
}
