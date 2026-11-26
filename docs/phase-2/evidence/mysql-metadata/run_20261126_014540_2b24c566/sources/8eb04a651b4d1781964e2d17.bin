//! 已有独立语义预期接入正式API；来源为固定29样例测试，未使用探针codec实现。
#![allow(clippy::unwrap_used)]
use seata_protocol::protocol::{
    RegisterRMRequest, RegisterRMResponse, RegisterTMRequest, RegisterTMResponse,
};
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchCommitRequest, BranchCommitResponse, BranchRegisterRequest,
    BranchRegisterResponse, BranchReportRequest, BranchReportResponse, BranchRollbackRequest,
    BranchRollbackResponse, GlobalBeginRequest, GlobalBeginResponse, GlobalCommitRequest,
    GlobalCommitResponse, GlobalLockQueryRequest, GlobalLockQueryResponse, GlobalReportRequest,
    GlobalReportResponse, GlobalRollbackRequest, GlobalRollbackResponse, GlobalStatusRequest,
    GlobalStatusResponse, UndoLogDeleteRequest,
};
use seata_protocol::{
    BranchStatus, BranchType, GlobalStatus, Message, PayloadCodec, ResultCode,
    TransactionExceptionCode,
};
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
            .join("../../tests/fixtures/protocol/v1")
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
            let codec = PayloadCodec::new(8 * 1024 * 1024).unwrap();
            let decoded = codec.decode(&bytes).unwrap();
            // 语义和值独立于待测编解码器及输入字节，不能只验证自己往返。
            assert_eq!(
                decoded, expected,
                "independent semantic contract: {}",
                $name
            );
            assert_eq!(
                codec.encode(&decoded).unwrap(),
                bytes,
                "exact Go payload: {}",
                $name
            );
            assert_eq!(
                codec.encode(&expected).unwrap(),
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
    Message::GlobalBeginRequest(GlobalBeginRequest {
        timeout: 2000,
        transaction_name: some("SeataGoTransaction")
    })
);

golden!(
    golden_global_begin_response,
    "GlobalBeginResponse",
    Message::GlobalBeginResponse(GlobalBeginResponse {
        response: success(),
        xid: some("127.0.0.1:8091:1"),
        extra_data: None
    })
);

golden!(
    golden_global_begin_response_failed,
    "GlobalBeginResponseFailed",
    Message::GlobalBeginResponse(GlobalBeginResponse {
        response: failed(TransactionExceptionCode::BeginFailed, "begin denied"),
        xid: None,
        extra_data: None
    })
);

golden!(
    golden_branch_commit_request,
    "BranchCommitRequest",
    Message::BranchCommitRequest(BranchCommitRequest {
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
    Message::BranchCommitResponse(BranchCommitResponse {
        response: success(),
        xid: some("127.0.0.1:8091:1"),
        branch_id: 0x0102030405060708,
        branch_status: BranchStatus::PhaseTwoCommitted
    })
);

golden!(
    golden_branch_commit_response_failed,
    "BranchCommitResponseFailed",
    Message::BranchCommitResponse(BranchCommitResponse {
        response: failed(TransactionExceptionCode::Io, "branch retry"),
        xid: some("127.0.0.1:8091:1"),
        branch_id: 0x0102030405060708,
        branch_status: BranchStatus::PhaseTwoCommitFailedRetryable
    })
);

golden!(
    golden_branch_rollback_request,
    "BranchRollbackRequest",
    Message::BranchRollbackRequest(BranchRollbackRequest {
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
    Message::BranchRollbackResponse(BranchRollbackResponse {
        response: success(),
        xid: some("127.0.0.1:8091:1"),
        branch_id: 0x0102030405060708,
        branch_status: BranchStatus::PhaseTwoRollbacked
    })
);

golden!(
    golden_global_commit_request,
    "GlobalCommitRequest",
    Message::GlobalCommitRequest(GlobalCommitRequest {
        xid: some("127.0.0.1:8091:1"),
        extra_data: None
    })
);

golden!(
    golden_global_commit_response,
    "GlobalCommitResponse",
    Message::GlobalCommitResponse(GlobalCommitResponse {
        response: success(),
        global_status: GlobalStatus::Committed
    })
);

golden!(
    golden_global_commit_response_failed,
    "GlobalCommitResponseFailed",
    Message::GlobalCommitResponse(GlobalCommitResponse {
        response: failed(TransactionExceptionCode::Io, "commit pending"),
        global_status: GlobalStatus::Committing
    })
);

golden!(
    golden_global_rollback_request,
    "GlobalRollbackRequest",
    Message::GlobalRollbackRequest(GlobalRollbackRequest {
        xid: some("127.0.0.1:8091:1"),
        extra_data: None
    })
);

golden!(
    golden_global_rollback_response,
    "GlobalRollbackResponse",
    Message::GlobalRollbackResponse(GlobalRollbackResponse {
        response: success(),
        global_status: GlobalStatus::Rollbacked
    })
);

golden!(
    golden_branch_register_request,
    "BranchRegisterRequest",
    Message::BranchRegisterRequest(BranchRegisterRequest {
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
    Message::BranchRegisterResponse(BranchRegisterResponse {
        response: success(),
        branch_id: 0x0102030405060708
    })
);

golden!(
    golden_branch_register_response_failed,
    "BranchRegisterResponseFailed",
    Message::BranchRegisterResponse(BranchRegisterResponse {
        response: failed(TransactionExceptionCode::LockKeyConflict, "lock conflict"),
        branch_id: 0
    })
);

golden!(
    golden_branch_report_request,
    "BranchReportRequest",
    Message::BranchReportRequest(BranchReportRequest {
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
    Message::BranchReportResponse(BranchReportResponse {
        response: success()
    })
);

golden!(
    golden_global_status_request,
    "GlobalStatusRequest",
    Message::GlobalStatusRequest(GlobalStatusRequest {
        xid: some("127.0.0.1:8091:1"),
        extra_data: None
    })
);

golden!(
    golden_global_status_response,
    "GlobalStatusResponse",
    Message::GlobalStatusResponse(GlobalStatusResponse {
        response: success(),
        global_status: GlobalStatus::Committed
    })
);

golden!(
    golden_global_report_request,
    "GlobalReportRequest",
    Message::GlobalReportRequest(GlobalReportRequest {
        xid: some("127.0.0.1:8091:1"),
        extra_data: None,
        global_status: Some(GlobalStatus::Begin)
    })
);

golden!(
    golden_global_report_response,
    "GlobalReportResponse",
    Message::GlobalReportResponse(GlobalReportResponse {
        response: success(),
        global_status: GlobalStatus::Begin
    })
);

golden!(
    golden_global_lock_query_request,
    "GlobalLockQueryRequest",
    Message::GlobalLockQueryRequest(GlobalLockQueryRequest {
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
    Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
        response: success(),
        lockable: true
    })
);

golden!(
    golden_register_tm_request,
    "RegisterTMRequest",
    Message::RegisterTMRequest(RegisterTMRequest {
        version: "V1,0".to_owned(),
        application_id: "orders-rust".to_owned(),
        transaction_service_group: "default_tx_group".to_owned(),
        extra_data: String::new()
    })
);

golden!(
    golden_register_tm_response,
    "RegisterTMResponse",
    Message::RegisterTMResponse(RegisterTMResponse {
        identified: true,
        wire_version: some("V1,0")
    })
);

golden!(
    golden_register_rm_request,
    "RegisterRMRequest",
    Message::RegisterRMRequest(RegisterRMRequest {
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
    Message::RegisterRMResponse(RegisterRMResponse {
        identified: true,
        wire_version: some("V1,0")
    })
);

golden!(
    golden_undo_log_delete_request,
    "UndoLogDeleteRequest",
    Message::UndoLogDeleteRequest(UndoLogDeleteRequest {
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
fn all_original_case_identities_are_required() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/protocol/v1");
    let mut actual: Vec<String> = std::fs::read_dir(root)
        .unwrap()
        .map(|r| r.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .map(|p| p.file_stem().unwrap().to_str().unwrap().to_owned())
        .collect();
    let mut expected = NAMES.map(str::to_owned);
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
