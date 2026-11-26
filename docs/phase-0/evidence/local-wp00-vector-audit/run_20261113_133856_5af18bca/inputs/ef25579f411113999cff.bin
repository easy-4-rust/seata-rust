// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
use super::abstract_transaction_response::AbstractTransactionResponse;
use super::branch_commit_response::BranchCommitResponse;
use super::branch_register_response::BranchRegisterResponse;
use super::branch_report_response::BranchReportResponse;
use super::branch_rollback_response::BranchRollbackResponse;
use super::branch_status::BranchStatus;
use super::global_begin_response::GlobalBeginResponse;
use super::global_commit_response::GlobalCommitResponse;
use super::global_lock_query_response::GlobalLockQueryResponse;
use super::global_report_response::GlobalReportResponse;
use super::global_rollback_response::GlobalRollbackResponse;
use super::global_status::GlobalStatus;
use super::global_status_response::GlobalStatusResponse;
use super::payload_message::PayloadMessage;
use super::result_code::ResultCode;
use super::transaction_exception_code::TransactionExceptionCode as Code;
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn all_java_exception_names_preserve_meaning_in_ten_response_families() {
    let errors = [
        ("Unknown", Code::Unknown),
        ("BeginFailed", Code::BeginFailed),
        ("LockKeyConflict", Code::LockKeyConflict),
        ("IO", Code::Io),
        (
            "BranchRollbackFailed_Retriable",
            Code::BranchRollbackFailedRetriable,
        ),
        (
            "BranchRollbackFailed_Unretriable",
            Code::BranchRollbackFailedUnretriable,
        ),
        ("BranchRegisterFailed", Code::BranchRegisterFailed),
        ("BranchReportFailed", Code::BranchReportFailed),
        ("LockableCheckFailed", Code::LockableCheckFailed),
        ("BranchTransactionNotExist", Code::BranchTransactionNotExist),
        ("GlobalTransactionNotExist", Code::GlobalTransactionNotExist),
        (
            "GlobalTransactionNotActive",
            Code::GlobalTransactionNotActive,
        ),
        (
            "GlobalTransactionStatusInvalid",
            Code::GlobalTransactionStatusInvalid,
        ),
        (
            "FailedToSendBranchCommitRequest",
            Code::FailedToSendBranchCommitRequest,
        ),
        (
            "FailedToSendBranchRollbackRequest",
            Code::FailedToSendBranchRollbackRequest,
        ),
        ("FailedToAddBranch", Code::FailedToAddBranch),
        (
            "FailedLockGlobalTransaction",
            Code::FailedLockGlobalTransaction,
        ),
        ("FailedWriteSession", Code::FailedWriteSession),
        ("FailedStore", Code::FailedStore),
        ("NotRaftLeader", Code::NotRaftLeader),
        ("LockKeyConflictFailFast", Code::LockKeyConflictFailFast),
        ("TransactionTimeout", Code::TransactionTimeout),
        ("CommitHeuristic", Code::CommitHeuristic),
        ("Broken", Code::Broken),
        ("BranchPrepareFailed", Code::BranchPrepareFailed),
    ];
    let directory = std::env::var_os("SEATA_IT_EXCEPTION_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/protocol/v1/exception_profile")
        });
    let text = std::fs::read_to_string(directory.join("vectors.tsv")).unwrap();
    let mut seen = HashSet::new();
    for line in text.lines() {
        let (name, hex) = line.split_once('\t').unwrap();
        assert!(seen.insert(name.to_owned()));
        let (kind, error_name) = name.split_once('_').unwrap();
        let code = errors
            .iter()
            .find(|(name, _)| *name == error_name)
            .unwrap()
            .1;
        let response = AbstractTransactionResponse {
            result_code: ResultCode::Failed,
            msg: Some(format!("wire:{error_name}")),
            transaction_exception_code: code,
        };
        let xid = Some("127.0.0.1:8091:33".to_owned());
        let expected = match kind {
            "2" => PayloadMessage::GlobalBeginResponse(GlobalBeginResponse {
                response,
                xid,
                extra_data: Some("meta".to_owned()),
            }),
            "4" => PayloadMessage::BranchCommitResponse(BranchCommitResponse {
                response,
                xid,
                branch_id: 0x0102030405060708,
                branch_status: BranchStatus::PhaseTwoCommitFailedRetryable,
            }),
            "6" => PayloadMessage::BranchRollbackResponse(BranchRollbackResponse {
                response,
                xid,
                branch_id: 0x0102030405060708,
                branch_status: BranchStatus::PhaseTwoRollbackFailedRetryable,
            }),
            "8" => PayloadMessage::GlobalCommitResponse(GlobalCommitResponse {
                response,
                global_status: GlobalStatus::RollbackRetrying,
            }),
            "10" => PayloadMessage::GlobalRollbackResponse(GlobalRollbackResponse {
                response,
                global_status: GlobalStatus::RollbackRetrying,
            }),
            "12" => PayloadMessage::BranchRegisterResponse(BranchRegisterResponse {
                response,
                branch_id: 0x0102030405060708,
            }),
            "14" => PayloadMessage::BranchReportResponse(BranchReportResponse { response }),
            "16" => PayloadMessage::GlobalStatusResponse(GlobalStatusResponse {
                response,
                global_status: GlobalStatus::RollbackRetrying,
            }),
            "18" => PayloadMessage::GlobalReportResponse(GlobalReportResponse {
                response,
                global_status: GlobalStatus::RollbackRetrying,
            }),
            "22" => PayloadMessage::GlobalLockQueryResponse(GlobalLockQueryResponse {
                response,
                lockable: false,
            }),
            _ => panic!("unknown exception fixture type"),
        };
        let (pairs, remainder) = hex.as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        let bytes: Vec<_> = pairs
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(PayloadMessage::decode(&bytes).unwrap(), expected, "{name}");
        assert_eq!(expected.encode().unwrap(), bytes, "{name}");
        let error_at = 5 + usize::from(u16::from_be_bytes([bytes[3], bytes[4]]));
        for bad_code in 25..=255 {
            let mut invalid = bytes.clone();
            invalid[error_at] = bad_code;
            assert!(
                PayloadMessage::decode(&invalid).is_err(),
                "{name}: {bad_code}"
            );
        }
    }
    for kind in [2, 4, 6, 8, 10, 12, 14, 16, 18, 22] {
        for (name, _) in errors {
            assert!(seen.contains(&format!("{kind}_{name}")));
        }
    }
    assert_eq!(seen.len(), 250);
}
