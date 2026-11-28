//! Java实际聚合载荷的正式API合同；不缓存原始输入作为对象。
#![allow(clippy::unwrap_used)]
use seata_protocol::protocol::{MergeResultMessage, MergedWarpMessage};
use seata_protocol::protocol::{RegisterRMResponse, RegisterTMResponse};
use seata_protocol::transaction::{
    AbstractTransactionResponse, GlobalBeginRequest, GlobalBeginResponse, GlobalCommitResponse,
    GlobalReportRequest, GlobalStatusRequest,
};
use seata_protocol::{
    GlobalStatus, Message, PayloadCodec, PayloadLimits, ProtocolError, ResultCode,
    TransactionExceptionCode,
};

fn codec() -> PayloadCodec {
    PayloadCodec::with_limits(PayloadLimits::new(64 * 1024, 512, 1024, 16).unwrap())
}
fn fixture(family: &str, name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/protocol/v1")
            .join(family)
            .join(format!("{name}.payload.bin")),
    )
    .unwrap()
}
fn request(msgs: Vec<Message>, msg_ids: Vec<i32>) -> Message {
    Message::MergedWarpMessage(MergedWarpMessage { msgs, msg_ids })
}
fn result(msgs: Vec<Message>) -> Message {
    Message::MergeResultMessage(MergeResultMessage { msgs: Some(msgs) })
}
fn begin() -> Message {
    Message::GlobalBeginRequest(GlobalBeginRequest {
        timeout: 2000,
        transaction_name: Some("m-a".into()),
    })
}
fn status() -> Message {
    Message::GlobalStatusRequest(GlobalStatusRequest {
        xid: Some("a".into()),
        extra_data: None,
    })
}
fn success() -> AbstractTransactionResponse {
    AbstractTransactionResponse {
        result_code: ResultCode::Success,
        msg: None,
        transaction_exception_code: TransactionExceptionCode::Unknown,
    }
}

#[test]
fn six_actual_java_control_merges_match_independent_fields_and_encoding() {
    let pairs = [
        (
            "MergeRequestWithIds",
            request(vec![begin(), status()], vec![7, -9]),
        ),
        (
            "MergeRequestWithoutIds",
            request(vec![begin(), status()], vec![]),
        ),
        ("MergeRequestEmpty", request(vec![], vec![])),
        (
            "MergeRequestNested",
            request(vec![request(vec![begin(), status()], vec![])], vec![42]),
        ),
        (
            "MergeResult",
            result(vec![
                Message::GlobalBeginResponse(GlobalBeginResponse {
                    response: success(),
                    xid: Some("a".into()),
                    extra_data: None,
                }),
                Message::GlobalCommitResponse(GlobalCommitResponse {
                    response: success(),
                    global_status: GlobalStatus::Committed,
                }),
            ]),
        ),
        ("MergeResultEmpty", result(vec![])),
    ];
    for (name, expected) in pairs {
        let bytes = fixture("control_envelopes", name);
        assert_eq!(codec().decode(&bytes).unwrap(), expected, "{name}");
        assert_eq!(codec().encode(&expected).unwrap(), bytes, "{name}");
    }
}

#[test]
fn invalid_id_shapes_non_results_and_unset_result_array_fail_both_directions() {
    for invalid in [
        request(vec![begin(), status()], vec![1]),
        request(vec![], vec![1]),
        result(vec![begin()]),
        result(vec![request(vec![], vec![])]),
        Message::MergeResultMessage(MergeResultMessage { msgs: None }),
    ] {
        assert!(matches!(
            codec().encode(&invalid),
            Err(ProtocolError::InvalidValue { .. })
        ));
    }
    let valid = fixture("control_envelopes", "MergeRequestWithoutIds");
    for tail in [vec![0], vec![0; 4], vec![0; 9]] {
        let mut invalid = valid.clone();
        invalid.extend(tail);
        let length = (invalid.len() - 6) as i32;
        invalid[2..6].copy_from_slice(&length.to_be_bytes());
        assert!(matches!(
            codec().decode(&invalid),
            Err(ProtocolError::InvalidValue { .. })
        ));
    }
    let mut invalid = fixture("control_envelopes", "MergeRequestWithoutIds");
    invalid[1] = 60;
    assert!(matches!(
        codec().decode(&invalid),
        Err(ProtocolError::InvalidValue { .. })
    ));
}

#[test]
fn configured_batch_node_depth_and_byte_limits_apply_to_encode_and_decode() {
    let nested = request(vec![request(vec![begin(), status()], vec![])], vec![42]);
    let bytes = fixture("control_envelopes", "MergeRequestNested");
    for limits in [
        PayloadLimits::new(1024, 1, 20, 8).unwrap(),
        PayloadLimits::new(1024, 8, 3, 8).unwrap(),
        PayloadLimits::new(1024, 8, 20, 1).unwrap(),
        PayloadLimits::new(bytes.len() - 1, 8, 20, 8).unwrap(),
    ] {
        let limited = PayloadCodec::with_limits(limits);
        assert!(matches!(
            limited.encode(&nested),
            Err(ProtocolError::LimitExceeded { .. })
        ));
        assert!(matches!(
            limited.decode(&bytes),
            Err(ProtocolError::LimitExceeded { .. })
        ));
    }
    let exact = PayloadCodec::with_limits(PayloadLimits::new(bytes.len(), 2, 4, 2).unwrap());
    assert_eq!(exact.encode(&nested).unwrap(), bytes);
    assert_eq!(exact.decode(&bytes).unwrap(), nested);
    assert!(PayloadLimits::new(1024, 8, 20, 65).is_err());
    assert!(PayloadLimits::new(1024, 32768, 20, 8).is_err());
    assert!(PayloadLimits::new(1024, 8, 0, 8).is_err());
}

#[test]
fn every_truncated_merge_prefix_sealed_length_and_signed_count_is_rejected() {
    for name in [
        "MergeRequestWithIds",
        "MergeRequestWithoutIds",
        "MergeRequestEmpty",
        "MergeRequestNested",
        "MergeResult",
        "MergeResultEmpty",
    ] {
        let valid = fixture("control_envelopes", name);
        for n in 0..valid.len() {
            assert!(codec().decode(&valid[..n]).is_err(), "{name} {n}");
        }
        for length in [-1, 0, 1, i32::MAX] {
            let mut invalid = valid.clone();
            invalid[2..6].copy_from_slice(&length.to_be_bytes());
            assert!(codec().decode(&invalid).is_err(), "{name} {length}");
        }
        let mut invalid = valid.clone();
        invalid[6..8].copy_from_slice(&(-1i16).to_be_bytes());
        assert!(matches!(
            codec().decode(&invalid),
            Err(ProtocolError::InvalidLength { .. })
        ));
        let mut trailing = valid;
        trailing.push(0);
        assert!(matches!(
            codec().decode(&trailing),
            Err(ProtocolError::TrailingBytes { .. })
        ));
    }
}

// 按固定Java MergedMessageCompatibilityTest构造组合关系；普通子对象由已被
// payload_golden独立字段预期验证的原29语料取得，不使用本批聚合输入生成其预期。
fn base_examples() -> Vec<Message> {
    [
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
    ]
    .into_iter()
    .map(|name| {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/protocol/v1")
                .join(format!("{name}.bin")),
        )
        .unwrap();
        codec().decode(&bytes).unwrap()
    })
    .collect()
}

fn profile_expected() -> std::collections::BTreeMap<String, Message> {
    let base = base_examples();
    let mut expected = std::collections::BTreeMap::new();
    for child in &base {
        let kind = child.message_type().to_java_v1();
        expected.insert(
            format!("Single_{kind}"),
            request(vec![child.clone()], vec![-100 - i32::from(kind)]),
        );
    }
    let ids = (0..25)
        .map(|i| if i % 2 == 0 { i + 1 } else { -i - 1 })
        .collect();
    let all = request(base.clone(), ids);
    let results = result(base.iter().filter(|m| m.is_result()).cloned().collect());
    expected.insert("All25WithIds".into(), all.clone());
    expected.insert("All25WithoutIds".into(), request(base, vec![]));
    expected.insert("All12Results".into(), results.clone());
    expected.insert(
        "NestedAll".into(),
        request(vec![all, results], vec![77, -77]),
    );
    let exceptions = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/protocol/v1/formal_payload_oracles/exceptions.tsv"),
    )
    .unwrap();
    let errors = exceptions
        .lines()
        .map(|line| {
            let value = line.split('\t').nth(3).unwrap();
            codec().decode(&unhex(value)).unwrap()
        })
        .collect();
    expected.insert("All250ExceptionResults".into(), result(errors));
    for timeout in [-1, i32::MIN, 0, i32::MAX] {
        expected.insert(
            format!("SignedTimeout_{timeout}"),
            request(
                vec![Message::GlobalBeginRequest(GlobalBeginRequest {
                    timeout,
                    transaction_name: Some("signed".into()),
                })],
                vec![],
            ),
        );
    }
    expected.insert(
        "AbsentTMVersion".into(),
        request(
            vec![Message::RegisterTMResponse(RegisterTMResponse {
                identified: false,
                wire_version: None,
            })],
            vec![],
        ),
    );
    expected.insert(
        "AbsentRMVersion".into(),
        request(
            vec![Message::RegisterRMResponse(RegisterRMResponse {
                identified: false,
                wire_version: None,
            })],
            vec![],
        ),
    );
    for (name, global_status) in [
        ("NullReportStatus", None),
        ("UnknownReportStatus", Some(GlobalStatus::Unknown)),
    ] {
        expected.insert(
            name.into(),
            request(
                vec![Message::GlobalReportRequest(GlobalReportRequest {
                    xid: Some("ctx".into()),
                    extra_data: Some("meta".into()),
                    global_status,
                })],
                vec![],
            ),
        );
    }
    expected
}
fn unhex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn all_38_java_profile_trees_match_composed_semantics_and_exact_bytes() {
    let mut expected = profile_expected();
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/protocol/v1/merge_profile/vectors.tsv"),
    )
    .unwrap();
    let mut count = 0;
    for line in text.lines() {
        let (name, value) = line.split_once('\t').unwrap();
        let wanted = expected.remove(name).unwrap();
        let bytes = fixture("merge_profile", name);
        assert_eq!(bytes, unhex(value));
        assert_eq!(codec().decode(&bytes).unwrap(), wanted, "{name}");
        assert_eq!(codec().encode(&wanted).unwrap(), bytes, "{name}");
        count += 1;
    }
    assert_eq!(count, 38);
    assert!(expected.is_empty());
}

#[test]
fn shared_node_budget_and_64_layer_stack_boundary_are_enforced() {
    let tree = request(
        vec![
            request(vec![begin()], vec![]),
            request(vec![status()], vec![]),
        ],
        vec![],
    );
    let bytes = codec().encode(&tree).unwrap();
    let limited = PayloadCodec::with_limits(PayloadLimits::new(4096, 8, 4, 8).unwrap());
    assert!(matches!(
        limited.encode(&tree),
        Err(ProtocolError::LimitExceeded {
            limit: "payload nodes",
            ..
        })
    ));
    assert!(matches!(
        limited.decode(&bytes),
        Err(ProtocolError::LimitExceeded {
            limit: "payload nodes",
            ..
        })
    ));
    let mut tree = begin();
    for _ in 0..64 {
        tree = request(vec![tree], vec![]);
    }
    let maximal = PayloadCodec::with_limits(PayloadLimits::new(4096, 1, 66, 64).unwrap());
    let bytes = maximal.encode(&tree).unwrap();
    assert_eq!(maximal.decode(&bytes).unwrap(), tree);
    let tree = request(vec![tree], vec![]);
    assert!(matches!(
        maximal.encode(&tree),
        Err(ProtocolError::LimitExceeded {
            limit: "merge depth",
            ..
        })
    ));
    // 在合法64层载荷外封装第65层；解码也必须先拒绝，不能依靠编码入口保护。
    let mut bytes65 = vec![0, 59];
    bytes65.extend_from_slice(&i32::try_from(bytes.len() + 2).unwrap().to_be_bytes());
    bytes65.extend_from_slice(&[0, 1]);
    bytes65.extend(bytes);
    assert!(matches!(
        maximal.decode(&bytes65),
        Err(ProtocolError::LimitExceeded {
            limit: "merge depth",
            ..
        })
    ));
    let ordinary_only = PayloadCodec::new(4096).unwrap();
    assert!(ordinary_only.decode(&[0, 59, 0, 0, 0, 2, 0, 0]).is_err());
    assert!(ordinary_only.encode(&request(vec![], vec![])).is_err());
}

#[test]
fn accepted_single_byte_mutations_roundtrip_without_partial_objects_or_panics() {
    let mut accepted = 0;
    let mut rejected = 0;
    for name in profile_expected().keys() {
        let bytes = fixture("merge_profile", name);
        for i in 0..bytes.len() {
            let mut changed = bytes.clone();
            changed[i] ^= 0x80;
            match codec().decode(&changed) {
                Ok(value) => {
                    assert_eq!(codec().encode(&value).unwrap(), changed, "{name} byte {i}");
                    accepted += 1;
                }
                Err(_) => {
                    rejected += 1;
                }
            }
        }
    }
    assert!(accepted > 0 && rejected > 0);
}

#[test]
fn result_child_budget_is_checked_before_scanning_child_types() {
    let limited = PayloadCodec::with_limits(PayloadLimits::new(1024, 0, 1, 1).unwrap());
    let invalid = result(vec![begin()]);
    assert!(matches!(
        limited.encode(&invalid),
        Err(ProtocolError::LimitExceeded {
            limit: "batch children",
            ..
        })
    ));
}
