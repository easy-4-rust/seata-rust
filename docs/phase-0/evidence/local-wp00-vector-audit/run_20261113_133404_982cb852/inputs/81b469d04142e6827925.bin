// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
use super::control_message::ControlMessage;
use super::global_begin_request::GlobalBeginRequest;
use super::global_report_request::GlobalReportRequest;
use super::global_status::GlobalStatus;
use super::merge_result_message::MergeResultMessage;
use super::merged_warp_message::MergedWarpMessage;
use super::payload_message::PayloadMessage;
use super::register_rm_response::RegisterRMResponse;
use super::register_tm_response::RegisterTMResponse;
use std::collections::HashSet;
use std::path::PathBuf;

fn unhex(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn message(value: PayloadMessage) -> ControlMessage {
    ControlMessage::Payload(Box::new(value))
}

#[test]
fn merged_java_corpus_composes_independently_verified_payloads_and_optional_metadata() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/protocol/v1");
    let names = [
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
    ];
    // 原29及250例已有独立类型字段断言；此处验证组合边界、顺序、ID及缺省语义，不重复造一套基础字段预期。
    let ordinary: Vec<_> = names
        .iter()
        .map(|name| {
            PayloadMessage::decode(&std::fs::read(root.join(format!("{name}.bin"))).unwrap())
                .unwrap()
        })
        .collect();
    let types: Vec<_> = ordinary
        .iter()
        .map(|item| {
            let data = item.encode().unwrap();
            u16::from_be_bytes([data[0], data[1]])
        })
        .collect();
    let result_items: Vec<_> = ordinary
        .iter()
        .zip(&types)
        .filter(|(_, kind)| {
            matches!(
                **kind,
                2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 | 18 | 22 | 102 | 104
            )
        })
        .map(|(item, _)| item.clone())
        .collect();
    assert_eq!(result_items.len(), 12);
    let all = MergedWarpMessage {
        msgs: ordinary.iter().cloned().map(message).collect(),
        msg_ids: (0..25)
            .map(|i| if i % 2 == 0 { i + 1 } else { -i - 1 })
            .collect(),
    };
    let dir = std::env::var_os("SEATA_IT_MERGE_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("merge_profile"));
    let exception_dir = std::env::var_os("SEATA_IT_EXCEPTION_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("exception_profile"));
    let exceptions: Vec<_> = std::fs::read_to_string(exception_dir.join("vectors.tsv"))
        .unwrap()
        .lines()
        .map(|line| PayloadMessage::decode(&unhex(line.split_once('\t').unwrap().1)).unwrap())
        .collect();
    assert_eq!(exceptions.len(), 250);
    let rows = std::fs::read_to_string(dir.join("vectors.tsv")).unwrap();
    let mut seen = HashSet::new();
    for line in rows.lines() {
        let (name, hex) = line.split_once('\t').unwrap();
        assert!(seen.insert(name.to_owned()));
        let expected = if let Some(code) = name.strip_prefix("Single_") {
            let kind: u16 = code.parse().unwrap();
            let index = types.iter().position(|n| *n == kind).unwrap();
            ControlMessage::MergedWarp(MergedWarpMessage {
                msgs: vec![message(ordinary[index].clone())],
                msg_ids: vec![-100 - i32::from(kind)],
            })
        } else if let Some(value) = name.strip_prefix("SignedTimeout_") {
            let timeout: i32 = value.parse().unwrap();
            ControlMessage::MergedWarp(MergedWarpMessage {
                msgs: vec![message(PayloadMessage::GlobalBeginRequest(
                    GlobalBeginRequest {
                        timeout,
                        transaction_name: Some("signed".to_owned()),
                    },
                ))],
                msg_ids: vec![],
            })
        } else {
            match name {
                "All25WithIds" => ControlMessage::MergedWarp(all.clone()),
                "All25WithoutIds" => ControlMessage::MergedWarp(MergedWarpMessage {
                    msgs: all.msgs.clone(),
                    msg_ids: vec![],
                }),
                "All12Results" => ControlMessage::MergeResult(MergeResultMessage {
                    msgs: result_items.clone(),
                }),
                "All250ExceptionResults" => ControlMessage::MergeResult(MergeResultMessage {
                    msgs: exceptions.clone(),
                }),
                "NestedAll" => ControlMessage::MergedWarp(MergedWarpMessage {
                    msgs: vec![
                        ControlMessage::MergedWarp(all.clone()),
                        ControlMessage::MergeResult(MergeResultMessage {
                            msgs: result_items.clone(),
                        }),
                    ],
                    msg_ids: vec![77, -77],
                }),
                "AbsentTMVersion" => ControlMessage::MergedWarp(MergedWarpMessage {
                    msgs: vec![message(PayloadMessage::RegisterTMResponse(
                        RegisterTMResponse {
                            identified: false,
                            wire_version: None,
                        },
                    ))],
                    msg_ids: vec![],
                }),
                "AbsentRMVersion" => ControlMessage::MergedWarp(MergedWarpMessage {
                    msgs: vec![message(PayloadMessage::RegisterRMResponse(
                        RegisterRMResponse {
                            identified: false,
                            wire_version: None,
                        },
                    ))],
                    msg_ids: vec![],
                }),
                "NullReportStatus" | "UnknownReportStatus" => {
                    ControlMessage::MergedWarp(MergedWarpMessage {
                        msgs: vec![message(PayloadMessage::GlobalReportRequest(
                            GlobalReportRequest {
                                xid: Some("ctx".to_owned()),
                                extra_data: Some("meta".to_owned()),
                                global_status: if name == "NullReportStatus" {
                                    None
                                } else {
                                    Some(GlobalStatus::Unknown)
                                },
                            },
                        ))],
                        msg_ids: vec![],
                    })
                }
                _ => panic!("unknown merge fixture"),
            }
        };
        let wire = unhex(hex);
        let actual = ControlMessage::decode(&wire).unwrap();
        assert_eq!(actual, expected, "{name}");
        assert_eq!(actual.encode().unwrap(), wire, "{name}");
        if name == "AbsentTMVersion" {
            let ControlMessage::MergedWarp(value) = actual else {
                unreachable!()
            };
            let ControlMessage::Payload(payload) = &value.msgs[0] else {
                unreachable!()
            };
            let PayloadMessage::RegisterTMResponse(response) = payload.as_ref() else {
                unreachable!()
            };
            assert_eq!(response.java_version("2.8.0-SNAPSHOT"), "2.8.0-SNAPSHOT");
        }
    }
    for kind in types {
        assert!(seen.contains(&format!("Single_{kind}")));
    }
    assert_eq!(seen.len(), 38);
}
