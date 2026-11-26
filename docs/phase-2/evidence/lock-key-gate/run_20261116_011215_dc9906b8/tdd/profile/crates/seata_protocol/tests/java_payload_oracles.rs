//! 已认证Java实际输出接入正式codec；预期域值显式给定，不从待测转换器生成。
#![allow(clippy::unwrap_used)]
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchCommitResponse, BranchRegisterResponse,
    BranchReportResponse, BranchRollbackResponse, GlobalBeginResponse, GlobalCommitResponse,
    GlobalLockQueryResponse, GlobalReportResponse, GlobalRollbackResponse, GlobalStatusResponse,
};
use seata_protocol::{
    BranchStatus, GlobalStatus, Message, PayloadCodec, ResultCode, TransactionExceptionCode,
};
use std::collections::BTreeSet;

const ERRORS: [TransactionExceptionCode; 25] = [
    TransactionExceptionCode::Unknown,
    TransactionExceptionCode::BeginFailed,
    TransactionExceptionCode::LockKeyConflict,
    TransactionExceptionCode::Io,
    TransactionExceptionCode::BranchRollbackFailedRetriable,
    TransactionExceptionCode::BranchRollbackFailedUnretriable,
    TransactionExceptionCode::BranchRegisterFailed,
    TransactionExceptionCode::BranchReportFailed,
    TransactionExceptionCode::LockableCheckFailed,
    TransactionExceptionCode::BranchTransactionNotExist,
    TransactionExceptionCode::GlobalTransactionNotExist,
    TransactionExceptionCode::GlobalTransactionNotActive,
    TransactionExceptionCode::GlobalTransactionStatusInvalid,
    TransactionExceptionCode::FailedToSendBranchCommitRequest,
    TransactionExceptionCode::FailedToSendBranchRollbackRequest,
    TransactionExceptionCode::FailedToAddBranch,
    TransactionExceptionCode::FailedLockGlobalTransaction,
    TransactionExceptionCode::FailedWriteSession,
    TransactionExceptionCode::FailedStore,
    TransactionExceptionCode::NotRaftLeader,
    TransactionExceptionCode::LockKeyConflictFailFast,
    TransactionExceptionCode::TransactionTimeout,
    TransactionExceptionCode::CommitHeuristic,
    TransactionExceptionCode::Broken,
    TransactionExceptionCode::BranchPrepareFailed,
];
const ERROR_NAMES: [&str; 25] = [
    "Unknown",
    "BeginFailed",
    "LockKeyConflict",
    "IO",
    "BranchRollbackFailed_Retriable",
    "BranchRollbackFailed_Unretriable",
    "BranchRegisterFailed",
    "BranchReportFailed",
    "LockableCheckFailed",
    "BranchTransactionNotExist",
    "GlobalTransactionNotExist",
    "GlobalTransactionNotActive",
    "GlobalTransactionStatusInvalid",
    "FailedToSendBranchCommitRequest",
    "FailedToSendBranchRollbackRequest",
    "FailedToAddBranch",
    "FailedLockGlobalTransaction",
    "FailedWriteSession",
    "FailedStore",
    "NotRaftLeader",
    "LockKeyConflictFailFast",
    "TransactionTimeout",
    "CommitHeuristic",
    "Broken",
    "BranchPrepareFailed",
];
const GLOBALS: [GlobalStatus; 21] = [
    GlobalStatus::Unknown,
    GlobalStatus::Begin,
    GlobalStatus::Committing,
    GlobalStatus::CommitRetrying,
    GlobalStatus::Rollbacking,
    GlobalStatus::RollbackRetrying,
    GlobalStatus::TimeoutRollbacking,
    GlobalStatus::TimeoutRollbackRetrying,
    GlobalStatus::AsyncCommitting,
    GlobalStatus::Committed,
    GlobalStatus::CommitFailed,
    GlobalStatus::Rollbacked,
    GlobalStatus::RollbackFailed,
    GlobalStatus::TimeoutRollbacked,
    GlobalStatus::TimeoutRollbackFailed,
    GlobalStatus::Finished,
    GlobalStatus::CommitRetryTimeout,
    GlobalStatus::RollbackRetryTimeout,
    GlobalStatus::Deleting,
    GlobalStatus::StopCommitOrCommitRetry,
    GlobalStatus::StopRollbackOrRollbackRetry,
];

fn hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
        .collect()
}
fn text(value: &str) -> String {
    String::from_utf8(hex(value)).unwrap()
}
fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/protocol/v1/formal_payload_oracles")
            .join(name),
    )
    .unwrap()
}
fn global(kind: u16, status: GlobalStatus, response: AbstractTransactionResponse) -> Message {
    match kind {
        8 => Message::GlobalCommitResponse(GlobalCommitResponse {
            response,
            global_status: status,
        }),
        10 => Message::GlobalRollbackResponse(GlobalRollbackResponse {
            response,
            global_status: status,
        }),
        16 => Message::GlobalStatusResponse(GlobalStatusResponse {
            response,
            global_status: status,
        }),
        18 => Message::GlobalReportResponse(GlobalReportResponse {
            response,
            global_status: status,
        }),
        _ => panic!("unexpected global family"),
    }
}
fn check(expected: Message, wire: &[u8]) {
    let codec = PayloadCodec::new(1024 * 1024).unwrap();
    let decoded = codec.decode(wire).unwrap();
    assert_eq!(decoded, expected);
    assert_eq!(codec.encode(&expected).unwrap(), wire);
    assert_eq!(codec.encode(&decoded).unwrap(), wire);
}

#[test]
fn all_250_java_exception_records_have_independent_fields_and_encoding() {
    let mut identities = BTreeSet::new();
    for line in fixture("exceptions.tsv").lines() {
        let row: Vec<_> = line.split('\t').collect();
        assert_eq!(row.len(), 4);
        let kind: u16 = row[0].parse().unwrap();
        let code: usize = row[1].parse().unwrap();
        assert!(code < 25);
        assert_eq!(row[2], ERROR_NAMES[code]);
        assert!(identities.insert((kind, code)));
        let response = AbstractTransactionResponse {
            result_code: ResultCode::Failed,
            msg: Some(format!("wire:{}", ERROR_NAMES[code])),
            transaction_exception_code: ERRORS[code],
        };
        let expected = match kind {
            2 => Message::GlobalBeginResponse(GlobalBeginResponse {
                response,
                xid: Some("127.0.0.1:8091:33".into()),
                extra_data: Some("meta".into()),
            }),
            4 => Message::BranchCommitResponse(BranchCommitResponse {
                response,
                xid: Some("127.0.0.1:8091:33".into()),
                branch_id: 0x0102030405060708,
                branch_status: BranchStatus::PhaseTwoCommitFailedRetryable,
            }),
            6 => Message::BranchRollbackResponse(BranchRollbackResponse {
                response,
                xid: Some("127.0.0.1:8091:33".into()),
                branch_id: 0x0102030405060708,
                branch_status: BranchStatus::PhaseTwoRollbackFailedRetryable,
            }),
            8 | 10 | 16 | 18 => global(kind, GlobalStatus::RollbackRetrying, response),
            12 => Message::BranchRegisterResponse(BranchRegisterResponse {
                response,
                branch_id: 0x0102030405060708,
            }),
            14 => Message::BranchReportResponse(BranchReportResponse { response }),
            22 => Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
                response,
                lockable: false,
            }),
            _ => panic!("unexpected response family"),
        };
        check(expected, &hex(row[3]));
    }
    let required: BTreeSet<_> = [2, 4, 6, 8, 10, 12, 14, 16, 18, 22]
        .into_iter()
        .flat_map(|kind| (0..25).map(move |code| (kind, code)))
        .collect();
    assert_eq!(identities, required);
}

#[test]
fn all_168_java_status_records_preserve_failed_result_independently() {
    let mut identities = BTreeSet::new();
    for line in fixture("states.tsv").lines() {
        let row: Vec<_> = line.split('\t').collect();
        assert_eq!(row.len(), 4);
        let kind: u16 = row[0].parse().unwrap();
        let status: usize = row[1].parse().unwrap();
        let result: u8 = row[2].parse().unwrap();
        assert!(status < 21 && result <= 1);
        assert!(identities.insert((kind, status, result)));
        let response = if result == 0 {
            AbstractTransactionResponse {
                result_code: ResultCode::Failed,
                msg: Some("状态诊断:IO".into()),
                transaction_exception_code: TransactionExceptionCode::Io,
            }
        } else {
            AbstractTransactionResponse {
                result_code: ResultCode::Success,
                msg: None,
                transaction_exception_code: TransactionExceptionCode::Unknown,
            }
        };
        check(global(kind, GLOBALS[status], response), &hex(row[3]));
    }
    let required: BTreeSet<_> = [8, 10, 16, 18]
        .into_iter()
        .flat_map(|kind| {
            (0..21).flat_map(move |status| [0, 1].map(|result| (kind, status, result)))
        })
        .collect();
    assert_eq!(identities, required);
}

#[test]
fn all_seven_java_supplemental_failures_match_without_rewriting_go_records() {
    let mut kinds = BTreeSet::new();
    for line in fixture("failures.tsv").lines() {
        let row: Vec<_> = line.split('\t').collect();
        assert_eq!(row.len(), 9);
        let kind: u16 = row[0].parse().unwrap();
        assert!(kinds.insert(kind));
        let error: usize = row[1].parse().unwrap();
        assert!(error < 25);
        let response = AbstractTransactionResponse {
            result_code: ResultCode::Failed,
            msg: Some(text(row[2])),
            transaction_exception_code: ERRORS[error],
        };
        let expected = match kind {
            6 => {
                assert_eq!(row[5], "9");
                Message::BranchRollbackResponse(BranchRollbackResponse {
                    response,
                    xid: Some(text(row[3])),
                    branch_id: row[4].parse().unwrap(),
                    branch_status: BranchStatus::PhaseTwoRollbackFailedRetryable,
                })
            }
            10 | 16 | 18 => global(kind, GLOBALS[row[6].parse::<usize>().unwrap()], response),
            12 => Message::BranchRegisterResponse(BranchRegisterResponse {
                response,
                branch_id: row[4].parse().unwrap(),
            }),
            14 => Message::BranchReportResponse(BranchReportResponse { response }),
            22 => {
                assert_eq!(row[7], "0");
                Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
                    response,
                    lockable: false,
                })
            }
            _ => panic!("unexpected supplemental family"),
        };
        check(expected, &hex(row[8]));
    }
    assert_eq!(kinds, BTreeSet::from([6, 10, 12, 14, 16, 18, 22]));
}
