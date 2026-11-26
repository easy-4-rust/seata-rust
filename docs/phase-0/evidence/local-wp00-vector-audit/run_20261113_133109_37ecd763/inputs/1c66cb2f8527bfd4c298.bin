// Copyright 2026 Seata-Rust contributors.
// Licensed under the Apache License, Version 2.0.

use super::abstract_transaction_response::AbstractTransactionResponse;
use super::branch_register_response::BranchRegisterResponse;
use super::branch_report_response::BranchReportResponse;
use super::branch_rollback_response::BranchRollbackResponse;
use super::branch_status::BranchStatus;
use super::global_lock_query_response::GlobalLockQueryResponse;
use super::global_report_response::GlobalReportResponse;
use super::global_rollback_response::GlobalRollbackResponse;
use super::global_status::GlobalStatus;
use super::global_status_response::GlobalStatusResponse;
use super::payload_message::PayloadMessage;
use super::result_code::ResultCode;
use super::transaction_exception_code::TransactionExceptionCode;

fn fixture(name: &str, language: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/protocol/v1/failure_variants")
            .join(format!("{name}.{language}.bin")),
    )
    .unwrap()
}

fn failed(code: TransactionExceptionCode, msg: &str) -> AbstractTransactionResponse {
    AbstractTransactionResponse {
        result_code: ResultCode::Failed,
        msg: Some(msg.to_owned()),
        transaction_exception_code: code,
    }
}

fn verify(name: &str, expected: PayloadMessage, divergence: Option<&str>) {
    let java = fixture(name, "java");
    let go = fixture(name, "go");
    assert_eq!(
        PayloadMessage::decode(&java).unwrap(),
        expected,
        "independent Java field contract: {name}"
    );
    assert_eq!(
        expected.encode().unwrap(),
        java,
        "normative Java bytes: {name}"
    );
    match divergence {
        None => {
            assert_eq!(go, java, "byte-compatible case {name}");
            assert_eq!(PayloadMessage::decode(&go).unwrap(), expected);
        }
        Some("error_text_length") => {
            assert_ne!(go, java, "known Go short8/Java short16 difference {name}");
            assert!(
                PayloadMessage::decode(&go).is_err(),
                "do not guess Go dialect for Java V1 payload"
            );
        }
        Some("exception_code_ordinal") => {
            let decoded = PayloadMessage::decode(&go).unwrap();
            assert_ne!(
                decoded, expected,
                "Go19 is not Java LockKeyConflictFailFast20"
            );
            assert_eq!(
                decoded,
                PayloadMessage::BranchRegisterResponse(BranchRegisterResponse {
                    response: failed(TransactionExceptionCode::NotRaftLeader, "fail fast"),
                    branch_id: 0,
                }),
                "a syntactically valid Go19 byte means NotRaftLeader to the Java decoder"
            );
        }
        _ => panic!("invalid test classification"),
    }
}

#[test]
fn failed_branch_rollback_preserves_retryable_branch_status() {
    verify(
        "BranchRollbackResponseFailed",
        PayloadMessage::BranchRollbackResponse(BranchRollbackResponse {
            response: failed(
                TransactionExceptionCode::BranchRollbackFailedRetriable,
                "rollback retry",
            ),
            xid: Some("127.0.0.1:8091:1".into()),
            branch_id: 0x0102030405060708,
            branch_status: BranchStatus::PhaseTwoRollbackFailedRetryable,
        }),
        None,
    );
}

#[test]
fn failed_global_rollback_preserves_retrying_global_status() {
    verify(
        "GlobalRollbackResponseFailed",
        PayloadMessage::GlobalRollbackResponse(GlobalRollbackResponse {
            response: failed(TransactionExceptionCode::Io, "rollback pending"),
            global_status: GlobalStatus::RollbackRetrying,
        }),
        None,
    );
}

#[test]
fn failed_branch_report_uses_java_short16_and_rejects_go_short8() {
    verify(
        "BranchReportResponseFailed",
        PayloadMessage::BranchReportResponse(BranchReportResponse {
            response: failed(
                TransactionExceptionCode::BranchReportFailed,
                "report denied",
            ),
        }),
        Some("error_text_length"),
    );
}

#[test]
fn failed_global_status_is_not_a_successful_unknown_status() {
    verify(
        "GlobalStatusResponseFailed",
        PayloadMessage::GlobalStatusResponse(GlobalStatusResponse {
            response: failed(TransactionExceptionCode::Io, "status unavailable"),
            global_status: GlobalStatus::Unknown,
        }),
        None,
    );
}

#[test]
fn failed_global_report_keeps_echoed_status_without_proving_mutation() {
    verify(
        "GlobalReportResponseFailed",
        PayloadMessage::GlobalReportResponse(GlobalReportResponse {
            response: failed(TransactionExceptionCode::Io, "report pending"),
            global_status: GlobalStatus::Rollbacking,
        }),
        None,
    );
}

#[test]
fn failed_lock_check_is_not_a_successful_false_and_rejects_go_short8() {
    verify(
        "GlobalLockQueryResponseFailed",
        PayloadMessage::GlobalLockQueryResponse(GlobalLockQueryResponse {
            response: failed(
                TransactionExceptionCode::LockableCheckFailed,
                "lock check failed",
            ),
            lockable: false,
        }),
        Some("error_text_length"),
    );
}

#[test]
fn lock_fail_fast_uses_java20_and_retains_go19_semantic_divergence() {
    verify(
        "BranchRegisterLockKeyConflictFailFast",
        PayloadMessage::BranchRegisterResponse(BranchRegisterResponse {
            response: failed(
                TransactionExceptionCode::LockKeyConflictFailFast,
                "fail fast",
            ),
            branch_id: 0,
        }),
        Some("exception_code_ordinal"),
    );
}

#[test]
fn added_java_failure_payloads_reject_all_truncations_and_trailing_bytes() {
    for name in [
        "BranchRollbackResponseFailed",
        "GlobalRollbackResponseFailed",
        "BranchReportResponseFailed",
        "GlobalStatusResponseFailed",
        "GlobalReportResponseFailed",
        "GlobalLockQueryResponseFailed",
        "BranchRegisterLockKeyConflictFailFast",
    ] {
        let mut bytes = fixture(name, "java");
        for end in 0..bytes.len() {
            assert!(
                PayloadMessage::decode(&bytes[..end]).is_err(),
                "{name} at {end}"
            );
        }
        bytes.push(0);
        assert!(
            PayloadMessage::decode(&bytes).is_err(),
            "{name} trailing bytes"
        );
    }
}
