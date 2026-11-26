// 测试中的unwrap让任何非预期错误直接暴露为失败；生产API不使用unwrap。
#![allow(clippy::unwrap_used)]

//! 固定 Java 的独立码值预期，不以 Rust 枚举声明顺序推导线缆值。
use seata_protocol::{
    BranchStatus, BranchType, CompressorType, GlobalStatus, MessageType, ProtocolError,
    RequestType, ResultCode, SerializerType, TransactionExceptionCode,
};

macro_rules! assert_codes {
    ($ty:ident, [$($variant:ident = $code:literal),+ $(,)?]) => {{
        let expected = [$(($ty::$variant, $code)),+];
        for (value, code) in expected {
            assert_eq!($ty::from_java_v1(code).unwrap(), value);
            assert_eq!(value.to_java_v1(), code);
        }
        for code in 0..=u8::MAX {
            if !expected.iter().any(|(_, known)| *known == code) {
                assert!(matches!($ty::from_java_v1(code), Err(ProtocolError::UnknownCode { .. })), "{} unknown {code}", stringify!($ty));
            }
        }
    }};
}

#[test]
fn global_status_has_all_21_explicit_java_codes() {
    assert_codes!(
        GlobalStatus,
        [
            Unknown = 0,
            Begin = 1,
            Committing = 2,
            CommitRetrying = 3,
            Rollbacking = 4,
            RollbackRetrying = 5,
            TimeoutRollbacking = 6,
            TimeoutRollbackRetrying = 7,
            AsyncCommitting = 8,
            Committed = 9,
            CommitFailed = 10,
            Rollbacked = 11,
            RollbackFailed = 12,
            TimeoutRollbacked = 13,
            TimeoutRollbackFailed = 14,
            Finished = 15,
            CommitRetryTimeout = 16,
            RollbackRetryTimeout = 17,
            Deleting = 18,
            StopCommitOrCommitRetry = 19,
            StopRollbackOrRollbackRetry = 20
        ]
    );
}

#[test]
fn branch_status_and_mode_do_not_default_unknown_codes_to_at() {
    assert_codes!(
        BranchType,
        [At = 0, Tcc = 1, Saga = 2, Xa = 3, SagaAnnotation = 4]
    );
    assert_codes!(
        BranchStatus,
        [
            Unknown = 0,
            Registered = 1,
            PhaseOneDone = 2,
            PhaseOneFailed = 3,
            PhaseOneTimeout = 4,
            PhaseTwoCommitted = 5,
            PhaseTwoCommitFailedRetryable = 6,
            PhaseTwoCommitFailedUnretryable = 7,
            PhaseTwoRollbacked = 8,
            PhaseTwoRollbackFailedRetryable = 9,
            PhaseTwoRollbackFailedUnretryable = 10,
            PhaseTwoCommitFailedXaerNotaRetryable = 11,
            PhaseTwoRollbackFailedXaerNotaRetryable = 12,
            PhaseOneReadOnly = 13,
            StopRetry = 14,
            PhaseOnePrepareFailed = 15
        ]
    );
}

#[test]
fn exception_codes_keep_java_raft_leader_and_fail_fast_distinct() {
    assert_codes!(
        TransactionExceptionCode,
        [
            Unknown = 0,
            BeginFailed = 1,
            LockKeyConflict = 2,
            Io = 3,
            BranchRollbackFailedRetriable = 4,
            BranchRollbackFailedUnretriable = 5,
            BranchRegisterFailed = 6,
            BranchReportFailed = 7,
            LockableCheckFailed = 8,
            BranchTransactionNotExist = 9,
            GlobalTransactionNotExist = 10,
            GlobalTransactionNotActive = 11,
            GlobalTransactionStatusInvalid = 12,
            FailedToSendBranchCommitRequest = 13,
            FailedToSendBranchRollbackRequest = 14,
            FailedToAddBranch = 15,
            FailedLockGlobalTransaction = 16,
            FailedWriteSession = 17,
            FailedStore = 18,
            NotRaftLeader = 19,
            LockKeyConflictFailFast = 20,
            TransactionTimeout = 21,
            CommitHeuristic = 22,
            Broken = 23,
            BranchPrepareFailed = 24
        ]
    );
}

#[test]
fn envelope_codes_and_known_extensions_are_explicit() {
    assert_codes!(
        RequestType,
        [
            RequestSync = 0,
            Response = 1,
            RequestOneway = 2,
            HeartbeatRequest = 3,
            HeartbeatResponse = 4
        ]
    );
    assert_codes!(ResultCode, [Failed = 0, Success = 1]);
    assert_codes!(
        SerializerType,
        [
            Seata = 1,
            Protobuf = 2,
            Kryo = 4,
            Fst = 8,
            Hessian = 22,
            Grpc = 40,
            Jackson = 50,
            Fury = 86,
            Fory = 87,
            Fastjson2 = 100
        ]
    );
    assert_codes!(
        CompressorType,
        [
            None = 0,
            Gzip = 1,
            Zip = 2,
            Sevenz = 3,
            Bzip2 = 4,
            Lz4 = 5,
            Deflater = 6,
            Zstd = 7
        ]
    );
}

#[test]
fn sparse_message_codes_and_known_unsupported_families_are_preserved() {
    let rows = [
        (MessageType::GlobalBeginRequest, 1),
        (MessageType::GlobalBeginResponse, 2),
        (MessageType::BranchCommitRequest, 3),
        (MessageType::BranchCommitResponse, 4),
        (MessageType::BranchRollbackRequest, 5),
        (MessageType::BranchRollbackResponse, 6),
        (MessageType::GlobalCommitRequest, 7),
        (MessageType::GlobalCommitResponse, 8),
        (MessageType::GlobalRollbackRequest, 9),
        (MessageType::GlobalRollbackResponse, 10),
        (MessageType::BranchRegisterRequest, 11),
        (MessageType::BranchRegisterResponse, 12),
        (MessageType::BranchReportRequest, 13),
        (MessageType::BranchReportResponse, 14),
        (MessageType::GlobalStatusRequest, 15),
        (MessageType::GlobalStatusResponse, 16),
        (MessageType::GlobalReportRequest, 17),
        (MessageType::GlobalReportResponse, 18),
        (MessageType::GlobalLockQueryRequest, 21),
        (MessageType::GlobalLockQueryResponse, 22),
        (MessageType::MergedWarp, 59),
        (MessageType::MergeResult, 60),
        (MessageType::RegisterTmRequest, 101),
        (MessageType::RegisterTmResponse, 102),
        (MessageType::RegisterRmRequest, 103),
        (MessageType::RegisterRmResponse, 104),
        (MessageType::UnregisterRmRequest, 105),
        (MessageType::UnregisterRmResponse, 106),
        (MessageType::UndoLogDeleteRequest, 111),
        (MessageType::Heartbeat, 120),
        (MessageType::BatchResult, 121),
    ];
    for (value, code) in rows {
        assert_eq!(MessageType::from_java_v1(code).unwrap(), value);
        assert_eq!(value.to_java_v1(), code);
    }
    for code in 0..=u16::MAX {
        if !rows.iter().any(|(_, known)| *known == code) {
            assert!(matches!(
                MessageType::from_java_v1(code),
                Err(ProtocolError::UnknownCode { .. })
            ));
        }
    }
    assert!(!MessageType::UnregisterRmRequest.in_v1_payload_allowlist());
    assert!(!MessageType::UnregisterRmResponse.in_v1_payload_allowlist());
    assert!(!MessageType::BatchResult.in_v1_payload_allowlist());
    assert!(!MessageType::Heartbeat.in_v1_payload_allowlist());
    assert!(MessageType::MergedWarp.in_v1_payload_allowlist());
    assert!(MessageType::MergeResult.in_v1_payload_allowlist());
    assert_eq!(
        rows.iter()
            .filter(|(m, _)| m.in_v1_payload_allowlist())
            .count(),
        27
    );
}
