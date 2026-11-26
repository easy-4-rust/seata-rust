//! 正式API的可选字段、错误结果及畸形输入合同。
#![allow(clippy::unwrap_used)]
use seata_protocol::protocol::{RegisterRMRequest, RegisterTMResponse};
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchReportResponse, GlobalBeginRequest, GlobalCommitResponse,
    GlobalLockQueryResponse, UndoLogDeleteRequest,
};
use seata_protocol::{
    GlobalStatus, Message, MessageType, PayloadCodec, ProtocolError, ResultCode,
    TransactionExceptionCode,
};

fn codec() -> PayloadCodec {
    PayloadCodec::new(8 * 1024 * 1024).unwrap()
}
fn failed() -> AbstractTransactionResponse {
    AbstractTransactionResponse {
        result_code: ResultCode::Failed,
        msg: Some("x".repeat(256)),
        transaction_exception_code: TransactionExceptionCode::BranchReportFailed,
    }
}

#[test]
fn every_original_payload_prefix_and_trailing_byte_is_rejected() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/protocol/v1");
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "bin") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        for length in 0..bytes.len() {
            assert!(
                codec().decode(&bytes[..length]).is_err(),
                "{} prefix {length}",
                path.display()
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            codec().decode(&trailing),
            Err(ProtocolError::TrailingBytes { remaining: 1 })
        ));
    }
}

#[test]
fn report_absence_255_is_distinct_from_unknown_0() {
    let absent = [0, 17, 0, 0, 0, 0, 255];
    let unknown = [0, 17, 0, 0, 0, 0, 0];
    let a = codec().decode(&absent).unwrap();
    let b = codec().decode(&unknown).unwrap();
    match &a {
        Message::GlobalReportRequest(value) => assert_eq!(value.global_status, None),
        _ => panic!("wrong type"),
    }
    match &b {
        Message::GlobalReportRequest(value) => {
            assert_eq!(value.global_status, Some(GlobalStatus::Unknown))
        }
        _ => panic!("wrong type"),
    }
    assert_ne!(a, b);
    assert_eq!(codec().encode(&a).unwrap(), absent);
    assert_eq!(codec().encode(&b).unwrap(), unknown);
    for value in 21..=254 {
        let mut invalid = absent;
        invalid[6] = value;
        assert!(matches!(
            codec().decode(&invalid),
            Err(ProtocolError::UnknownCode { .. })
        ));
    }
}

#[test]
fn registration_wire_absence_and_java_getter_projection_are_separate() {
    for kind in [102, 104] {
        let bytes = [0, kind, 0, 0, 0];
        let value = codec().decode(&bytes).unwrap();
        match &value {
            Message::RegisterTMResponse(v) => {
                assert!(!v.identified);
                assert_eq!(v.wire_version, None);
                assert_eq!(v.java_version("2.8.0-SNAPSHOT"), "2.8.0-SNAPSHOT");
            }
            Message::RegisterRMResponse(v) => {
                assert!(!v.identified);
                assert_eq!(v.wire_version, None);
                assert_eq!(v.java_version("other-build"), "other-build");
            }
            _ => panic!("wrong type"),
        }
        assert_eq!(codec().encode(&value).unwrap(), bytes);
        let invalid = [0, kind, 2, 0, 0];
        assert!(matches!(
            codec().decode(&invalid),
            Err(ProtocolError::InvalidBoolean { .. })
        ));
    }
    let empty = RegisterTMResponse {
        identified: true,
        wire_version: Some(String::new()),
    };
    assert_eq!(empty.java_version("build-default"), "build-default");
    let tm = codec().decode(&[0, 101, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
    match tm {
        Message::RegisterTMRequest(v) => {
            assert_eq!(v.extra_data, "");
            assert_eq!(v.application_id, "");
        }
        _ => panic!("wrong type"),
    }
}

#[test]
fn failed_result_and_long_error_text_survive_each_response_layout() {
    let report = Message::BranchReportResponse(BranchReportResponse { response: failed() });
    let wire = codec().encode(&report).unwrap();
    assert_eq!(&wire[..5], &[0, 14, 0, 1, 0]);
    assert_eq!(codec().decode(&wire).unwrap(), report);
    let lock = Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
        response: failed(),
        lockable: true,
    });
    let wire = codec().encode(&lock).unwrap();
    assert_eq!(&wire[..5], &[0, 22, 0, 1, 0]);
    assert_eq!(&wire[wire.len() - 2..], &[0, 1]);
    let decoded = codec().decode(&wire).unwrap();
    assert_eq!(decoded, lock);
    assert_eq!(
        decoded.transaction_response().unwrap().result_code,
        ResultCode::Failed
    );
    let end = Message::GlobalCommitResponse(GlobalCommitResponse {
        response: failed(),
        global_status: GlobalStatus::Committed,
    });
    let decoded = codec().decode(&codec().encode(&end).unwrap()).unwrap();
    assert_eq!(decoded, end);
    assert_eq!(
        decoded.transaction_response().unwrap().result_code,
        ResultCode::Failed
    );
}

#[test]
fn unrepresentable_success_text_and_required_empty_fields_fail() {
    let invalid = Message::BranchReportResponse(BranchReportResponse {
        response: AbstractTransactionResponse {
            result_code: ResultCode::Success,
            msg: Some("hidden".into()),
            transaction_exception_code: TransactionExceptionCode::Unknown,
        },
    });
    assert!(matches!(
        codec().encode(&invalid),
        Err(ProtocolError::InvalidValue { .. })
    ));
    let invalid = Message::RegisterRMRequest(RegisterRMRequest {
        version: String::new(),
        application_id: None,
        transaction_service_group: "group".into(),
        extra_data: None,
        resource_ids: None,
    });
    assert!(matches!(
        codec().encode(&invalid),
        Err(ProtocolError::InvalidValue { .. })
    ));
    assert!(matches!(
        codec().decode(&[0, 103, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        Err(ProtocolError::InvalidValue { .. })
    ));
}

#[test]
fn malformed_utf8_huge_negative_lengths_and_unknown_codes_fail() {
    assert!(matches!(
        codec().decode(&[0, 1, 0, 0, 0, 1, 0, 1, 255]),
        Err(ProtocolError::InvalidUtf8 { .. })
    ));
    assert!(matches!(
        codec().decode(&[0, 11, 0, 0, 0, 0, 0, 127, 255, 255, 255]),
        Err(ProtocolError::Truncated { .. })
    ));
    assert!(matches!(
        codec().decode(&[0, 11, 0, 0, 0, 0, 0, 255, 255, 255, 255]),
        Err(ProtocolError::InvalidLength { .. })
    ));
    for kind in [19u16, 20, 23, 65535] {
        assert!(matches!(
            codec().decode(&kind.to_be_bytes()),
            Err(ProtocolError::UnknownCode { .. })
        ));
    }
    for kind in [105u16, 106, 120, 121] {
        assert!(
            matches!(codec().decode(&kind.to_be_bytes()),Err(ProtocolError::UnsupportedMessage{code}) if code==kind)
        );
    }
    for status in 21..=255 {
        assert!(matches!(
            codec().decode(&[0, 16, 1, 0, status]),
            Err(ProtocolError::UnknownCode { .. })
        ));
    }
}

#[test]
fn byte_limits_and_signed_wire_scalars_are_explicit() {
    assert!(matches!(
        PayloadCodec::new(1),
        Err(ProtocolError::InvalidConfiguration { .. })
    ));
    assert!(matches!(
        PayloadCodec::new(2).unwrap().decode(&[0, 1, 0]),
        Err(ProtocolError::LimitExceeded { .. })
    ));
    let request = Message::GlobalBeginRequest(GlobalBeginRequest {
        timeout: i32::MIN,
        transaction_name: Some("事务🦀".into()),
    });
    let wire = codec().encode(&request).unwrap();
    assert_eq!(&wire[..8], &[0, 1, 128, 0, 0, 0, 0, 10]);
    assert_eq!(codec().decode(&wire).unwrap(), request);
    assert!(matches!(
        PayloadCodec::new(2).unwrap().encode(&request),
        Err(ProtocolError::LimitExceeded { .. })
    ));
    let request = Message::UndoLogDeleteRequest(UndoLogDeleteRequest {
        branch_type: seata_protocol::BranchType::At,
        resource_id: "db".into(),
        save_days: -2,
    });
    let wire = codec().encode(&request).unwrap();
    assert_eq!(&wire[wire.len() - 2..], &[255, 254]);
    assert_eq!(codec().decode(&wire).unwrap(), request);
    assert!(PayloadCodec::supports(MessageType::MergedWarp));
    assert!(PayloadCodec::supports(MessageType::MergeResult));
    assert!(PayloadCodec::supports(MessageType::GlobalBeginRequest));
}

#[test]
fn debug_does_not_print_registration_secrets_or_error_text() {
    let request = Message::RegisterRMRequest(RegisterRMRequest {
        version: "V1,0".into(),
        application_id: Some("secret-token".into()),
        transaction_service_group: "group".into(),
        extra_data: Some("secret-token".into()),
        resource_ids: Some("user:secret-token@tcp(db)/data".into()),
    });
    assert!(!format!("{request:?}").contains("secret-token"));
    let response = Message::BranchReportResponse(BranchReportResponse {
        response: AbstractTransactionResponse {
            result_code: ResultCode::Failed,
            msg: Some("secret-token".into()),
            transaction_exception_code: TransactionExceptionCode::Io,
        },
    });
    assert!(!format!("{response:?}").contains("secret-token"));
}
