// Copyright 2026 Seata-Rust contributors.
// SPDX-License-Identifier: Apache-2.0

use super::control_frame::ControlFrame;
use super::control_message::ControlMessage;
use super::global_status::GlobalStatus;
use super::merge_result_message::MergeResultMessage;
use super::merged_warp_message::MergedWarpMessage;
use super::payload_message::PayloadMessage;
use super::result_code::ResultCode;

fn hex(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn bodyless_heartbeat_and_header_map_preserve_original_full_frames() {
    for (text, kind, headers) in [
        ("dada0100000010001003010010203040", 3, 0),
        ("dada0100000010001004010010203040", 4, 0),
        (
            "dada010000001d001d0301001020304000077472616365496400026862",
            3,
            1,
        ),
    ] {
        let bytes = hex(text);
        let frame = ControlFrame::decode(&bytes).unwrap();
        assert_eq!(frame.request_id, 0x10203040);
        assert_eq!(frame.kind, kind);
        assert_eq!(frame.head_map.len(), headers);
        assert!(frame.body.is_none());
        if headers == 1 {
            assert_eq!(
                frame.head_map[0],
                ("traceId".to_owned(), Some("hb".to_owned()))
            );
        }
        assert_eq!(frame.encode().unwrap(), bytes);
    }
}

#[test]
fn merged_request_retains_typed_children_and_signed_request_ids() {
    let bytes=hex("dada0100000032001000010010203040003b0000001c00020001000007d000036d2d61000f000161000000000007fffffff7");
    let frame = ControlFrame::decode(&bytes).unwrap();
    let ControlMessage::MergedWarp(batch) = frame.body.as_ref().unwrap() else {
        panic!("expected actual merged request")
    };
    assert_eq!(batch.msg_ids, [7, -9]);
    assert_eq!(batch.msgs.len(), 2);
    let ControlMessage::Payload(child) = &batch.msgs[0] else {
        panic!("expected typed leaf")
    };
    let PayloadMessage::GlobalBeginRequest(begin) = child.as_ref() else {
        panic!("expected begin")
    };
    assert_eq!(begin.timeout, 2000);
    assert_eq!(begin.transaction_name.as_deref(), Some("m-a"));
    assert_eq!(frame.encode().unwrap(), bytes);
}

fn fixtures() -> Vec<(String, Vec<u8>, Vec<u8>)> {
    let directory = std::env::var_os("SEATA_IT_CONTROL_WIRE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/protocol/v1/control_envelopes")
        });
    let input = std::fs::read_to_string(directory.join("vectors.tsv")).unwrap();
    let rows = input
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 3);
            (fields[0].to_owned(), hex(fields[1]), hex(fields[2]))
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 9);
    assert_eq!(
        rows.iter()
            .map(|r| &r.0)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        9
    );
    rows
}

#[test]
fn all_captured_java_control_frames_match_independent_fields_and_new_encoding() {
    for (name, bytes, payload) in fixtures() {
        let frame = ControlFrame::decode(&bytes).unwrap();
        assert_eq!(frame.request_id, 0x10203040);
        assert_eq!(frame.encode().unwrap(), bytes);
        if name.starts_with("Heartbeat") {
            assert!(payload.is_empty() && frame.body.is_none());
            assert_eq!(frame.kind, if name == "HeartbeatPong" { 4 } else { 3 });
            continue;
        }
        let body = frame.body.as_ref().unwrap();
        assert_eq!(body.encode().unwrap(), payload);
        match body {
            ControlMessage::MergedWarp(batch) => {
                assert_eq!(frame.kind, 0);
                if name == "MergeRequestEmpty" {
                    assert!(batch.msgs.is_empty() && batch.msg_ids.is_empty());
                } else if name == "MergeRequestNested" {
                    assert_eq!(batch.msg_ids, [42]);
                    assert_eq!(batch.msgs.len(), 1);
                    let ControlMessage::MergedWarp(inner) = &batch.msgs[0] else {
                        panic!("expected actual nested batch")
                    };
                    assert_eq!(inner.msgs.len(), 2);
                    assert!(inner.msg_ids.is_empty());
                } else {
                    assert_eq!(batch.msgs.len(), 2);
                    assert_eq!(
                        batch.msg_ids,
                        if name == "MergeRequestWithIds" {
                            vec![7, -9]
                        } else {
                            vec![]
                        }
                    );
                    let ControlMessage::Payload(child) = &batch.msgs[1] else {
                        panic!("expected status child")
                    };
                    let PayloadMessage::GlobalStatusRequest(status) = child.as_ref() else {
                        panic!("expected status request")
                    };
                    assert_eq!(status.xid.as_deref(), Some("a"));
                    assert!(status.extra_data.is_none());
                }
            }
            ControlMessage::MergeResult(batch) => {
                assert_eq!(frame.kind, 1);
                assert_eq!(
                    batch.msgs.len(),
                    if name == "MergeResultEmpty" { 0 } else { 2 }
                );
                if !batch.msgs.is_empty() {
                    let PayloadMessage::GlobalBeginResponse(begin) = &batch.msgs[0] else {
                        panic!("expected begin result")
                    };
                    assert_eq!(begin.xid.as_deref(), Some("a"));
                    assert!(begin.extra_data.is_none());
                    assert_eq!(begin.response.result_code, ResultCode::Success);
                    let PayloadMessage::GlobalCommitResponse(commit) = &batch.msgs[1] else {
                        panic!("expected commit result")
                    };
                    assert_eq!(commit.global_status, GlobalStatus::Committed);
                    assert_eq!(commit.response.result_code, ResultCode::Success);
                }
            }
            _ => panic!("unexpected plain top-level fixture"),
        }
    }
}

#[test]
fn truncated_trailing_and_hostile_control_headers_are_rejected() {
    for (_, bytes, _) in fixtures() {
        for end in 0..bytes.len() {
            assert!(ControlFrame::decode(&bytes[..end]).is_err());
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(ControlFrame::decode(&extra).is_err());
        for (offset, value) in [(0, 0), (2, 2), (7, 0xff), (9, 9), (10, 0xff), (11, 0xff)] {
            let mut changed = bytes.clone();
            changed[offset] = value;
            assert!(ControlFrame::decode(&changed).is_err());
        }
    }
}

#[test]
fn partial_child_ids_wrong_result_kind_and_extra_inner_bytes_are_rejected() {
    let (_, _, request) = fixtures()
        .into_iter()
        .find(|r| r.0 == "MergeRequestWithIds")
        .unwrap();
    let mut missing = request[..request.len() - 2].to_vec();
    let n = (missing.len() - 6) as u32;
    missing[2..6].copy_from_slice(&n.to_be_bytes());
    assert!(ControlMessage::decode(&missing).is_err());
    let mut extra = request.clone();
    extra.push(0);
    let n = (extra.len() - 6) as u32;
    extra[2..6].copy_from_slice(&n.to_be_bytes());
    assert!(ControlMessage::decode(&extra).is_err());
    for count in [[0xff, 0xff], [0x04, 0x01]] {
        let mut changed = request.clone();
        changed[6..8].copy_from_slice(&count);
        assert!(ControlMessage::decode(&changed).is_err());
    }
    let (_, _, mut result) = fixtures()
        .into_iter()
        .find(|r| r.0 == "MergeResult")
        .unwrap();
    result[8..10].copy_from_slice(&1u16.to_be_bytes());
    assert!(ControlMessage::decode(&result).is_err());
    let invalid = ControlMessage::MergeResult(MergeResultMessage {
        msgs: vec![PayloadMessage::decode(&hex("0001000007d000036d2d61")).unwrap()],
    });
    assert!(invalid.encode().is_err());
}

#[test]
fn nesting_and_total_message_budgets_apply_to_both_decode_and_encode() {
    let mut tree = ControlMessage::MergedWarp(MergedWarpMessage {
        msgs: vec![],
        msg_ids: vec![],
    });
    let mut raw = hex("003b000000020000");
    for _ in 0..8 {
        tree = ControlMessage::MergedWarp(MergedWarpMessage {
            msgs: vec![tree],
            msg_ids: vec![],
        });
        let mut next = vec![0, 59];
        next.extend_from_slice(&((raw.len() + 2) as u32).to_be_bytes());
        next.extend_from_slice(&[0, 1]);
        next.extend(raw);
        raw = next;
    }
    assert!(tree.encode().is_ok());
    assert!(ControlMessage::decode(&raw).is_ok());
    tree = ControlMessage::MergedWarp(MergedWarpMessage {
        msgs: vec![tree],
        msg_ids: vec![],
    });
    let mut next = vec![0, 59];
    next.extend_from_slice(&((raw.len() + 2) as u32).to_be_bytes());
    next.extend_from_slice(&[0, 1]);
    next.extend(raw);
    assert!(tree.encode().is_err());
    assert!(ControlMessage::decode(&next).is_err());
    let leaf = ControlMessage::Payload(Box::new(PayloadMessage::decode(&[0, 14, 1, 0]).unwrap()));
    let group = ControlMessage::MergedWarp(MergedWarpMessage {
        msgs: vec![leaf; 1024],
        msg_ids: vec![],
    });
    let group_bytes = group.encode().unwrap();
    let over = ControlMessage::MergedWarp(MergedWarpMessage {
        msgs: vec![group.clone(), group],
        msg_ids: vec![],
    });
    assert!(over.encode().is_err());
    let mut bytes = vec![0, 59];
    bytes.extend_from_slice(&((group_bytes.len() * 2 + 2) as u32).to_be_bytes());
    bytes.extend_from_slice(&[0, 2]);
    bytes.extend_from_slice(&group_bytes);
    bytes.extend_from_slice(&group_bytes);
    assert!(ControlMessage::decode(&bytes).is_err());
}

#[test]
fn header_null_empty_duplicate_and_heartbeat_body_are_explicit() {
    let mut frame = ControlFrame {
        request_id: u32::MAX,
        kind: 3,
        head_map: vec![
            ("nullable".to_owned(), None),
            (String::new(), Some(String::new())),
        ],
        body: None,
    };
    let bytes = frame.encode().unwrap();
    assert_eq!(ControlFrame::decode(&bytes).unwrap(), frame);
    frame
        .head_map
        .push(("nullable".to_owned(), Some("other".to_owned())));
    assert!(frame.encode().is_err());
    let mut body = hex("dada010000001100100301001020304000");
    assert!(ControlFrame::decode(&body).is_err());
    body[9] = 0;
    assert!(ControlFrame::decode(&body).is_err());
    let duplicate = hex("dada010000001a001a0301001020304000016100000001610000");
    assert!(ControlFrame::decode(&duplicate).is_err());
    let null_key = hex("dada0100000014001403010010203040ffff0000");
    assert!(ControlFrame::decode(&null_key).is_err());
}

#[test]
fn every_single_byte_control_mutation_is_rejected_or_preserved_without_panic() {
    for (_, bytes, _) in fixtures() {
        for offset in 0..bytes.len() {
            for byte in 0..=u8::MAX {
                if bytes[offset] == byte {
                    continue;
                }
                let mut changed = bytes.clone();
                changed[offset] = byte;
                if let Ok(frame) = ControlFrame::decode(&changed) {
                    assert_eq!(
                        frame.encode().unwrap(),
                        changed,
                        "accepted control mutation must preserve all bytes"
                    );
                }
            }
        }
    }
}
