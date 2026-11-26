// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements. See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License. You may obtain a copy of the License at
// http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::abstract_transaction_response::AbstractTransactionResponse;
use super::branch_commit_request::BranchCommitRequest;
use super::branch_commit_response::BranchCommitResponse;
use super::branch_register_request::BranchRegisterRequest;
use super::branch_register_response::BranchRegisterResponse;
use super::branch_report_request::BranchReportRequest;
use super::branch_report_response::BranchReportResponse;
use super::branch_rollback_request::BranchRollbackRequest;
use super::branch_rollback_response::BranchRollbackResponse;
use super::branch_status::BranchStatus;
use super::branch_type::BranchType;
use super::global_begin_request::GlobalBeginRequest;
use super::global_begin_response::GlobalBeginResponse;
use super::global_commit_request::GlobalCommitRequest;
use super::global_commit_response::GlobalCommitResponse;
use super::global_lock_query_request::GlobalLockQueryRequest;
use super::global_lock_query_response::GlobalLockQueryResponse;
use super::global_report_request::GlobalReportRequest;
use super::global_report_response::GlobalReportResponse;
use super::global_rollback_request::GlobalRollbackRequest;
use super::global_rollback_response::GlobalRollbackResponse;
use super::global_status::GlobalStatus;
use super::global_status_request::GlobalStatusRequest;
use super::global_status_response::GlobalStatusResponse;
use super::payload_message::PayloadMessage;
use super::register_rm_request::RegisterRMRequest;
use super::register_rm_response::RegisterRMResponse;
use super::register_tm_request::RegisterTMRequest;
use super::register_tm_response::RegisterTMResponse;
use super::result_code::ResultCode;
use super::transaction_exception_code::TransactionExceptionCode;
use super::undo_log_delete_request::UndoLogDeleteRequest;
use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;

fn some(value: &str) -> Option<String> {
    Some(value.to_owned())
}

fn success() -> AbstractTransactionResponse {
    AbstractTransactionResponse {
        result_code: ResultCode::Success,
        msg: None,
        transaction_exception_code: TransactionExceptionCode::Unknown,
    }
}

fn failed(code: TransactionExceptionCode, msg: &str) -> AbstractTransactionResponse {
    AbstractTransactionResponse {
        result_code: ResultCode::Failed,
        msg: some(msg),
        transaction_exception_code: code,
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/protocol/v1")
            .join(format!("{name}.bin")),
    )
    .unwrap()
}

macro_rules! golden {
    ($test:ident, $name:literal, $expected:expr) => {
        #[test]
        fn $test() {
            let bytes = fixture($name);
            let expected = $expected;
            let decoded = PayloadMessage::decode(&bytes).unwrap();
            // 语义和值独立于待测编解码器及输入字节，不能只验证自己往返。
            assert_eq!(
                decoded, expected,
                "independent semantic contract: {}",
                $name
            );
            assert_eq!(
                decoded.encode().unwrap(),
                bytes,
                "exact Go payload: {}",
                $name
            );
            assert_eq!(
                expected.encode().unwrap(),
                bytes,
                "independent encoding: {}",
                $name
            );
        }
    };
}

golden!(
    golden_global_begin_request,
    "GlobalBeginRequest",
    PayloadMessage::GlobalBeginRequest(GlobalBeginRequest {
        timeout: 2000,
        transaction_name: some("SeataGoTransaction")
    })
);

golden!(
    golden_global_begin_response,
    "GlobalBeginResponse",
    PayloadMessage::GlobalBeginResponse(GlobalBeginResponse {
        response: success(),
        xid: some("127.0.0.1:8091:1"),
        extra_data: None
    })
);

golden!(
    golden_global_begin_response_failed,
    "GlobalBeginResponseFailed",
    PayloadMessage::GlobalBeginResponse(GlobalBeginResponse {
        response: failed(TransactionExceptionCode::BeginFailed, "begin denied"),
        xid: None,
        extra_data: None
    })
);

golden!(
    golden_branch_commit_request,
    "BranchCommitRequest",
    PayloadMessage::BranchCommitRequest(BranchCommitRequest {
        xid: "127.0.0.1:8091:1".to_owned(),
        branch_id: 0x0102030405060708,
        branch_type: BranchType::At,
        resource_id: "mysql://orders".to_owned(),
        application_data: None
    })
);

golden!(
    golden_branch_commit_response,
    "BranchCommitResponse",
    PayloadMessage::BranchCommitResponse(BranchCommitResponse {
        response: success(),
        xid: some("127.0.0.1:8091:1"),
        branch_id: 0x0102030405060708,
        branch_status: BranchStatus::PhaseTwoCommitted
    })
);

golden!(
    golden_branch_commit_response_failed,
    "BranchCommitResponseFailed",
    PayloadMessage::BranchCommitResponse(BranchCommitResponse {
        response: failed(TransactionExceptionCode::Io, "branch retry"),
        xid: some("127.0.0.1:8091:1"),
        branch_id: 0x0102030405060708,
        branch_status: BranchStatus::PhaseTwoCommitFailedRetryable
    })
);

golden!(
    golden_branch_rollback_request,
    "BranchRollbackRequest",
    PayloadMessage::BranchRollbackRequest(BranchRollbackRequest {
        xid: "127.0.0.1:8091:1".to_owned(),
        branch_id: 0x0102030405060708,
        branch_type: BranchType::At,
        resource_id: "mysql://orders".to_owned(),
        application_data: None
    })
);

golden!(
    golden_branch_rollback_response,
    "BranchRollbackResponse",
    PayloadMessage::BranchRollbackResponse(BranchRollbackResponse {
        response: success(),
        xid: some("127.0.0.1:8091:1"),
        branch_id: 0x0102030405060708,
        branch_status: BranchStatus::PhaseTwoRollbacked
    })
);

golden!(
    golden_global_commit_request,
    "GlobalCommitRequest",
    PayloadMessage::GlobalCommitRequest(GlobalCommitRequest {
        xid: some("127.0.0.1:8091:1"),
        extra_data: None
    })
);

golden!(
    golden_global_commit_response,
    "GlobalCommitResponse",
    PayloadMessage::GlobalCommitResponse(GlobalCommitResponse {
        response: success(),
        global_status: GlobalStatus::Committed
    })
);

golden!(
    golden_global_commit_response_failed,
    "GlobalCommitResponseFailed",
    PayloadMessage::GlobalCommitResponse(GlobalCommitResponse {
        response: failed(TransactionExceptionCode::Io, "commit pending"),
        global_status: GlobalStatus::Committing
    })
);

golden!(
    golden_global_rollback_request,
    "GlobalRollbackRequest",
    PayloadMessage::GlobalRollbackRequest(GlobalRollbackRequest {
        xid: some("127.0.0.1:8091:1"),
        extra_data: None
    })
);

golden!(
    golden_global_rollback_response,
    "GlobalRollbackResponse",
    PayloadMessage::GlobalRollbackResponse(GlobalRollbackResponse {
        response: success(),
        global_status: GlobalStatus::Rollbacked
    })
);

golden!(
    golden_branch_register_request,
    "BranchRegisterRequest",
    PayloadMessage::BranchRegisterRequest(BranchRegisterRequest {
        xid: some("127.0.0.1:8091:1"),
        branch_type: BranchType::At,
        resource_id: some("mysql://orders"),
        lock_key: some("orders:42"),
        application_data: None
    })
);

golden!(
    golden_branch_register_response,
    "BranchRegisterResponse",
    PayloadMessage::BranchRegisterResponse(BranchRegisterResponse {
        response: success(),
        branch_id: 0x0102030405060708
    })
);

golden!(
    golden_branch_register_response_failed,
    "BranchRegisterResponseFailed",
    PayloadMessage::BranchRegisterResponse(BranchRegisterResponse {
        response: failed(TransactionExceptionCode::LockKeyConflict, "lock conflict"),
        branch_id: 0
    })
);

golden!(
    golden_branch_report_request,
    "BranchReportRequest",
    PayloadMessage::BranchReportRequest(BranchReportRequest {
        xid: some("127.0.0.1:8091:1"),
        branch_id: 0x0102030405060708,
        status: BranchStatus::PhaseOneDone,
        resource_id: some("mysql://orders"),
        application_data: some("trace=abc"),
        branch_type: BranchType::At
    })
);

golden!(
    golden_branch_report_response,
    "BranchReportResponse",
    PayloadMessage::BranchReportResponse(BranchReportResponse {
        response: success()
    })
);

golden!(
    golden_global_status_request,
    "GlobalStatusRequest",
    PayloadMessage::GlobalStatusRequest(GlobalStatusRequest {
        xid: some("127.0.0.1:8091:1"),
        extra_data: None
    })
);

golden!(
    golden_global_status_response,
    "GlobalStatusResponse",
    PayloadMessage::GlobalStatusResponse(GlobalStatusResponse {
        response: success(),
        global_status: GlobalStatus::Committed
    })
);

golden!(
    golden_global_report_request,
    "GlobalReportRequest",
    PayloadMessage::GlobalReportRequest(GlobalReportRequest {
        xid: some("127.0.0.1:8091:1"),
        extra_data: None,
        global_status: Some(GlobalStatus::Begin)
    })
);

golden!(
    golden_global_report_response,
    "GlobalReportResponse",
    PayloadMessage::GlobalReportResponse(GlobalReportResponse {
        response: success(),
        global_status: GlobalStatus::Begin
    })
);

golden!(
    golden_global_lock_query_request,
    "GlobalLockQueryRequest",
    PayloadMessage::GlobalLockQueryRequest(GlobalLockQueryRequest {
        xid: some("127.0.0.1:8091:1"),
        branch_type: BranchType::At,
        resource_id: some("mysql://orders"),
        lock_key: some("orders:42"),
        application_data: None
    })
);

golden!(
    golden_global_lock_query_response,
    "GlobalLockQueryResponse",
    PayloadMessage::GlobalLockQueryResponse(GlobalLockQueryResponse {
        response: success(),
        lockable: true
    })
);

golden!(
    golden_register_tm_request,
    "RegisterTMRequest",
    PayloadMessage::RegisterTMRequest(RegisterTMRequest {
        version: "V1,0".to_owned(),
        application_id: "orders-rust".to_owned(),
        transaction_service_group: "default_tx_group".to_owned(),
        extra_data: String::new()
    })
);

golden!(
    golden_register_tm_response,
    "RegisterTMResponse",
    PayloadMessage::RegisterTMResponse(RegisterTMResponse {
        identified: true,
        wire_version: some("V1,0")
    })
);

golden!(
    golden_register_rm_request,
    "RegisterRMRequest",
    PayloadMessage::RegisterRMRequest(RegisterRMRequest {
        version: "V1,0".to_owned(),
        application_id: some("orders-rust"),
        transaction_service_group: "default_tx_group".to_owned(),
        extra_data: None,
        resource_ids: some("mysql://orders")
    })
);

golden!(
    golden_register_rm_response,
    "RegisterRMResponse",
    PayloadMessage::RegisterRMResponse(RegisterRMResponse {
        identified: true,
        wire_version: some("V1,0")
    })
);

golden!(
    golden_undo_log_delete_request,
    "UndoLogDeleteRequest",
    PayloadMessage::UndoLogDeleteRequest(UndoLogDeleteRequest {
        branch_type: BranchType::At,
        resource_id: "mysql://orders".to_owned(),
        save_days: 7
    })
);

const NAMES: [&str; 29] = [
    "GlobalBeginRequest",
    "GlobalBeginResponse",
    "BranchCommitRequest",
    "BranchCommitResponse",
    "BranchRollbackRequest",
    "BranchRollbackResponse",
    "GlobalCommitRequest",
    "GlobalCommitResponse",
    "GlobalRollbackRequest",
    "GlobalRollbackResponse",
    "BranchRegisterRequest",
    "BranchRegisterResponse",
    "BranchReportRequest",
    "BranchReportResponse",
    "GlobalStatusRequest",
    "GlobalStatusResponse",
    "GlobalReportRequest",
    "GlobalReportResponse",
    "GlobalLockQueryRequest",
    "GlobalLockQueryResponse",
    "RegisterTMRequest",
    "RegisterTMResponse",
    "RegisterRMRequest",
    "RegisterRMResponse",
    "UndoLogDeleteRequest",
    "GlobalBeginResponseFailed",
    "GlobalCommitResponseFailed",
    "BranchRegisterResponseFailed",
    "BranchCommitResponseFailed",
];

#[test]
fn java_global_report_null_status_is_a_valid_sentinel() {
    let bytes = [0, 17, 0, 0, 0, 0, 0xff];
    let message = PayloadMessage::decode(&bytes).expect("Java encodes a null report status as -1");
    assert_eq!(message.encode().unwrap(), bytes);
    assert_eq!(
        message,
        PayloadMessage::GlobalReportRequest(GlobalReportRequest {
            xid: None,
            extra_data: None,
            global_status: None,
        })
    );
    for code in 21..=254 {
        let mut unknown = bytes;
        unknown[6] = code;
        assert!(
            PayloadMessage::decode(&unknown).is_err(),
            "unknown report status {code}"
        );
    }
}

#[test]
fn java_registration_missing_version_keeps_constructor_default() {
    for code in [102, 104] {
        let bytes = [0, code, 0, 0, 0];
        let message = PayloadMessage::decode(&bytes).unwrap();
        let version = match &message {
            PayloadMessage::RegisterTMResponse(value) => Some(value.java_version("2.8.0-SNAPSHOT")),
            PayloadMessage::RegisterRMResponse(value) => Some(value.java_version("2.8.0-SNAPSHOT")),
            _ => unreachable!(),
        };
        assert_eq!(
            version,
            Some("2.8.0-SNAPSHOT"),
            "Java retains its constructor default"
        );
        assert_eq!(
            message.encode().unwrap(),
            bytes,
            "wire absence remains distinguishable"
        );
    }
    for name in ["RegisterTMResponse", "RegisterRMResponse"] {
        let message = PayloadMessage::decode(&fixture(name)).unwrap();
        let version = match &message {
            PayloadMessage::RegisterTMResponse(value) => value.java_version("2.8.0-SNAPSHOT"),
            PayloadMessage::RegisterRMResponse(value) => value.java_version("2.8.0-SNAPSHOT"),
            _ => unreachable!(),
        };
        assert_eq!(
            version, "V1,0",
            "present wire value overrides Java constructor default"
        );
    }
}

#[test]
fn all_29_go_payloads_are_decoded() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/protocol/v1");
    let mut actual: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "bin"))
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    let mut expected = NAMES.map(str::to_owned);
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    for name in NAMES {
        assert!(PayloadMessage::decode(&fixture(name)).is_ok(), "{name}");
    }
}

#[test]
fn every_truncated_prefix_and_trailing_byte_is_rejected() {
    for name in NAMES {
        let mut bytes = fixture(name);
        for end in 0..bytes.len() {
            assert!(
                PayloadMessage::decode(&bytes[..end]).is_err(),
                "{name}, truncated at {end}"
            );
        }
        bytes.push(0);
        assert!(
            PayloadMessage::decode(&bytes).is_err(),
            "{name}, trailing data"
        );
    }
}

#[test]
fn every_unknown_enum_and_message_type_is_rejected() {
    for code in 0..=u8::MAX {
        assert_eq!(ResultCode::from_code(code).is_ok(), code <= 1);
        assert_eq!(BranchType::from_code(code).is_ok(), code <= 4);
        assert_eq!(BranchStatus::from_code(code).is_ok(), code <= 15);
        assert_eq!(GlobalStatus::from_code(code).is_ok(), code <= 20);
        assert_eq!(
            TransactionExceptionCode::from_code(code).is_ok(),
            code <= 24
        );
        // 所有已知码必须保留原码，防止枚举位置互换后仍被误判为有效。
        if code <= 1 {
            assert_eq!(ResultCode::from_code(code).unwrap() as u8, code);
        }
        if code <= 4 {
            assert_eq!(BranchType::from_code(code).unwrap() as u8, code);
        }
        if code <= 15 {
            assert_eq!(BranchStatus::from_code(code).unwrap() as u8, code);
        }
        if code <= 20 {
            assert_eq!(GlobalStatus::from_code(code).unwrap() as u8, code);
        }
        if code <= 24 {
            assert_eq!(
                TransactionExceptionCode::from_code(code).unwrap() as u8,
                code
            );
        }
    }
    let known: Vec<u16> = NAMES
        .iter()
        .map(|name| u16::from_be_bytes(fixture(name)[..2].try_into().unwrap()))
        .collect();
    for code in 0..=u16::MAX {
        if !known.contains(&code) {
            assert!(
                PayloadMessage::decode(&code.to_be_bytes()).is_err(),
                "type {code}"
            );
        }
    }
    // 从真实载荷入口验证枚举，不只测试辅助枚举函数。
    for (name, at, first_bad) in [
        ("GlobalBeginResponse", 2, 2),
        ("GlobalBeginResponse", 3, 25),
        ("GlobalCommitResponse", 4, 21),
        ("BranchCommitResponse", 30, 16),
        ("BranchCommitRequest", 28, 5),
        ("RegisterTMResponse", 2, 2),
    ] {
        for invalid in first_bad..=u8::MAX {
            let mut bytes = fixture(name);
            bytes[at] = invalid;
            assert!(
                PayloadMessage::decode(&bytes).is_err(),
                "{name} field {at}, {invalid}"
            );
        }
    }
    let mut bytes = fixture("GlobalLockQueryResponse");
    bytes[4..6].copy_from_slice(&2_u16.to_be_bytes());
    assert!(PayloadMessage::decode(&bytes).is_err());
}

#[test]
fn malformed_utf8_lengths_and_missing_required_fields_are_rejected() {
    for (name, at) in [
        ("GlobalBeginRequest", 8),
        ("GlobalBeginResponseFailed", 5),
        ("RegisterTMRequest", 4),
        ("BranchReportRequest", 49),
    ] {
        let mut bytes = fixture(name);
        bytes[at] = 0xff;
        assert!(PayloadMessage::decode(&bytes).is_err(), "UTF-8 {name}");
    }
    for bytes in [
        vec![0, 1, 0, 0, 7, 208, 0xff, 0xff], // short 长度为 -1
        vec![0, 1, 0, 0, 7, 208, 0x7f, 0xff], // 合法长度却无内容
        vec![0, 103, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // RM version 为空，Java 返回部分对象
        vec![0, 3, 0, 0],                     // 回调没有 XID
        vec![0, 111, 0, 0, 0, 0, 7],          // UndoDelete 无 resource
    ] {
        assert!(PayloadMessage::decode(&bytes).is_err());
    }
    let original = fixture("BranchRegisterRequest");
    for declared in [
        u32::MAX,
        0x80000000,
        0x7fffffff,
        crate::MAX_FRAME_LEN as u32,
    ] {
        let mut bytes = original.clone();
        bytes[37..41].copy_from_slice(&declared.to_be_bytes());
        assert!(
            PayloadMessage::decode(&bytes).is_err(),
            "string32 {declared}"
        );
    }
    assert!(PayloadMessage::decode(&vec![0; crate::MAX_FRAME_LEN + 1]).is_err());
}

#[test]
fn every_single_byte_mutation_is_rejected_or_preserved_without_panicking() {
    for name in NAMES {
        let original = fixture(name);
        let mut mutated = original.clone();
        for at in 0..original.len() {
            for byte in 0..=u8::MAX {
                mutated[at] = byte;
                if let Ok(message) = PayloadMessage::decode(&mutated) {
                    assert_eq!(
                        message.encode().unwrap(),
                        mutated,
                        "accepted mutation {name}[{at}]={byte}"
                    );
                }
            }
            mutated[at] = original[at];
        }
    }
}

#[test]
fn zero_length_semantics_match_each_java_codec() {
    let tm = PayloadMessage::RegisterTMRequest(RegisterTMRequest {
        version: String::new(),
        application_id: String::new(),
        transaction_service_group: String::new(),
        extra_data: String::new(),
    });
    assert_eq!(tm.encode().unwrap(), vec![0, 101, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(PayloadMessage::decode(&tm.encode().unwrap()).unwrap(), tm);
    let rm = PayloadMessage::RegisterRMRequest(RegisterRMRequest {
        version: "v".into(),
        application_id: None,
        transaction_service_group: String::new(),
        extra_data: None,
        resource_ids: None,
    });
    assert_eq!(PayloadMessage::decode(&rm.encode().unwrap()).unwrap(), rm);
    let begin = PayloadMessage::GlobalBeginResponse(GlobalBeginResponse {
        response: failed(TransactionExceptionCode::BeginFailed, ""),
        xid: None,
        extra_data: None,
    });
    let mut expected = begin.clone();
    if let PayloadMessage::GlobalBeginResponse(message) = &mut expected {
        message.response.msg = None;
    }
    assert_eq!(
        PayloadMessage::decode(&begin.encode().unwrap()).unwrap(),
        expected
    );
    for message in [
        PayloadMessage::GlobalBeginRequest(GlobalBeginRequest {
            timeout: 0,
            transaction_name: None,
        }),
        PayloadMessage::RegisterTMResponse(RegisterTMResponse {
            identified: false,
            wire_version: None,
        }),
        PayloadMessage::RegisterRMResponse(RegisterRMResponse {
            identified: false,
            wire_version: None,
        }),
        PayloadMessage::GlobalLockQueryResponse(GlobalLockQueryResponse {
            response: success(),
            lockable: false,
        }),
        PayloadMessage::GlobalCommitRequest(GlobalCommitRequest {
            xid: None,
            extra_data: None,
        }),
    ] {
        assert_eq!(
            PayloadMessage::decode(&message.encode().unwrap()).unwrap(),
            message
        );
    }
}

#[test]
fn signed_scalars_and_multibyte_text_are_preserved() {
    for message in [
        PayloadMessage::GlobalBeginRequest(GlobalBeginRequest {
            timeout: i32::MIN,
            transaction_name: some("订单😀"),
        }),
        PayloadMessage::BranchReportRequest(BranchReportRequest {
            xid: None,
            branch_id: i64::MIN,
            status: BranchStatus::PhaseOnePrepareFailed,
            resource_id: some("数据库"),
            application_data: some("跟踪=😀"),
            branch_type: BranchType::Xa,
        }),
        PayloadMessage::UndoLogDeleteRequest(UndoLogDeleteRequest {
            branch_type: BranchType::SagaAnnotation,
            resource_id: "db".into(),
            save_days: i16::MIN,
        }),
        PayloadMessage::GlobalCommitResponse(GlobalCommitResponse {
            response: failed(TransactionExceptionCode::TransactionTimeout, "等待回滚😀"),
            global_status: GlobalStatus::RollbackRetryTimeout,
        }),
    ] {
        assert_eq!(
            PayloadMessage::decode(&message.encode().unwrap()).unwrap(),
            message
        );
    }
}

#[test]
fn writer_checks_byte_length_and_payload_limit_before_truncation() {
    let valid = PayloadMessage::GlobalBeginRequest(GlobalBeginRequest {
        timeout: 0,
        transaction_name: some(&"a".repeat(i16::MAX as usize)),
    });
    assert_eq!(
        PayloadMessage::decode(&valid.encode().unwrap()).unwrap(),
        valid
    );
    for text in ["a".repeat(i16::MAX as usize + 1), "中".repeat(10923)] {
        let too_long = PayloadMessage::GlobalBeginRequest(GlobalBeginRequest {
            timeout: 0,
            transaction_name: Some(text),
        });
        assert!(too_long.encode().is_err());
    }
    let too_large = PayloadMessage::BranchRegisterRequest(BranchRegisterRequest {
        xid: None,
        branch_type: BranchType::At,
        resource_id: None,
        lock_key: some(&"a".repeat(crate::MAX_FRAME_LEN)),
        application_data: None,
    });
    assert!(too_large.encode().is_err());
    let invalid_success = PayloadMessage::BranchReportResponse(BranchReportResponse {
        response: AbstractTransactionResponse {
            result_code: ResultCode::Success,
            msg: some("not on wire"),
            transaction_exception_code: TransactionExceptionCode::Unknown,
        },
    });
    assert!(invalid_success.encode().is_err());
    for name in ["BranchCommitRequest", "BranchRollbackRequest"] {
        let mut value = PayloadMessage::decode(&fixture(name)).unwrap();
        match &mut value {
            PayloadMessage::BranchCommitRequest(v) => v.xid.clear(),
            PayloadMessage::BranchRollbackRequest(v) => v.resource_id.clear(),
            _ => unreachable!(),
        }
        assert!(value.encode().is_err());
    }
    let mut writer = WireWriter::default();
    writer
        .optional32(Some(&"a".repeat(crate::MAX_FRAME_LEN - 4)))
        .unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes.len(), crate::MAX_FRAME_LEN);
    let mut reader = WireReader::new(&bytes).unwrap();
    assert_eq!(
        reader.optional32().unwrap().unwrap().len(),
        crate::MAX_FRAME_LEN - 4
    );
    reader.finish().unwrap();
}
