//! 完整V1帧合同：原Java/Go帧与合成畸形边界分别核对。
#![allow(clippy::unwrap_used)]
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::GlobalBeginRequest;
use seata_protocol::v1::{FrameLimits, ProtocolDecoderV1, ProtocolEncoderV1};
use seata_protocol::{
    CompressorType, Message, PayloadLimits, ProtocolError, RequestType, SerializerType,
};

fn limits() -> FrameLimits {
    FrameLimits::new(
        64 * 1024,
        32767,
        128,
        PayloadLimits::new(64 * 1024, 512, 1024, 16).unwrap(),
    )
    .unwrap()
}
fn ordinary() -> RpcMessage {
    RpcMessage {
        id: 1,
        message_type: RequestType::RequestSync,
        codec: SerializerType::Seata,
        compressor: CompressorType::None,
        head_map: vec![],
        body: Some(Message::GlobalBeginRequest(GlobalBeginRequest {
            timeout: 2000,
            transaction_name: Some("SeataGoTransaction".into()),
        })),
    }
}
fn unhex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
#[test]
fn original_begin_frames_match_independent_objects_with_and_without_headers() {
    for (headers,text) in [
        (vec![],"dada010000002a0010000100000000010001000007d000125365617461476f5472616e73616374696f6e"),
        (vec![("traceId".into(),Some("trace-001".into()))],"dada010000003e002400010000000001000774726163654964000974726163652d3030310001000007d000125365617461476f5472616e73616374696f6e")
    ] {
        let mut expected=ordinary();expected.head_map=headers;
        let bytes=unhex(text);
        assert_eq!(ProtocolEncoderV1::new(limits()).encode(&expected).unwrap(),bytes);
        assert_eq!(ProtocolDecoderV1::new(limits()).decode(&bytes).unwrap(),expected);
    }
}
#[test]
fn all_nine_java_control_frames_include_empty_heartbeat_and_full_merge_bodies() {
    for name in [
        "HeartbeatPing",
        "HeartbeatPong",
        "HeartbeatHeadMap",
        "MergeRequestWithIds",
        "MergeRequestWithoutIds",
        "MergeRequestEmpty",
        "MergeRequestNested",
        "MergeResult",
        "MergeResultEmpty",
    ] {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/protocol/v1/control_envelopes")
                .join(format!("{name}.frame.bin")),
        )
        .unwrap();
        let decoded = ProtocolDecoderV1::new(limits()).decode(&bytes).unwrap();
        assert_eq!(decoded.id, 0x10203040);
        assert_eq!(decoded.codec, SerializerType::Seata);
        assert_eq!(decoded.compressor, CompressorType::None);
        assert_eq!(decoded.body.is_none(), name.starts_with("Heartbeat"));
        assert_eq!(
            ProtocolEncoderV1::new(limits()).encode(&decoded).unwrap(),
            bytes,
            "{name}"
        );
    }
}
#[test]
fn ordered_headers_preserve_null_empty_unicode_and_signed_request_ids() {
    let mut frame = ordinary();
    frame.id = i32::MIN;
    frame.head_map = vec![
        ("z".into(), None),
        ("".into(), Some("".into())),
        ("a".into(), Some("中文🦀".into())),
    ];
    let bytes = ProtocolEncoderV1::new(limits()).encode(&frame).unwrap();
    assert_eq!(&bytes[12..16], &i32::MIN.to_be_bytes());
    assert_eq!(&bytes[16..25], &[0, 1, b'z', 255, 255, 0, 0, 0, 0]);
    assert_eq!(
        ProtocolDecoderV1::new(limits()).decode(&bytes).unwrap(),
        frame
    );
    frame.head_map.push(("z".into(), Some("other".into())));
    assert!(matches!(
        ProtocolEncoderV1::new(limits()).encode(&frame),
        Err(ProtocolError::InvalidValue { .. })
    ));
}
#[test]
fn malformed_frame_lengths_magic_versions_and_truncation_are_rejected() {
    let bytes = ProtocolEncoderV1::new(limits())
        .encode(&ordinary())
        .unwrap();
    let decoder = ProtocolDecoderV1::new(limits());
    for n in 0..bytes.len() {
        assert!(decoder.decode(&bytes[..n]).is_err(), "prefix {n}");
    }
    for (index, value) in [(0, 0), (1, 0), (2, 0), (2, 2), (9, 5), (10, 255), (11, 255)] {
        let mut bad = bytes.clone();
        bad[index] = value;
        assert!(decoder.decode(&bad).is_err(), "{index}/{value}");
    }
    for length in [0u32, 15, 16, 41, 43, u32::MAX] {
        let mut bad = bytes.clone();
        bad[3..7].copy_from_slice(&length.to_be_bytes());
        assert!(decoder.decode(&bad).is_err());
    }
    for length in [0u16, 15, 43, 32768, 65535] {
        let mut bad = bytes.clone();
        bad[7..9].copy_from_slice(&length.to_be_bytes());
        assert!(decoder.decode(&bad).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decoder.decode(&trailing).is_err());
    let mut coalesced = bytes.clone();
    coalesced.extend(bytes);
    assert!(decoder.decode(&coalesced).is_err());
}
#[test]
fn heartbeat_and_ordinary_body_rules_and_known_unsupported_codecs_are_explicit() {
    let mut frame = ordinary();
    for kind in [
        RequestType::HeartbeatRequest,
        RequestType::HeartbeatResponse,
    ] {
        frame.message_type = kind;
        assert!(ProtocolEncoderV1::new(limits()).encode(&frame).is_err());
        frame.body = None;
        let bytes = ProtocolEncoderV1::new(limits()).encode(&frame).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(
            ProtocolDecoderV1::new(limits()).decode(&bytes).unwrap(),
            frame
        );
        let mut with_body = bytes;
        with_body[3..7].copy_from_slice(&18u32.to_be_bytes());
        with_body.extend([0, 120]);
        assert!(ProtocolDecoderV1::new(limits()).decode(&with_body).is_err());
        frame.body = ordinary().body;
    }
    frame = ordinary();
    frame.body = None;
    assert!(ProtocolEncoderV1::new(limits()).encode(&frame).is_err());
    frame = ordinary();
    frame.codec = SerializerType::Protobuf;
    assert!(matches!(
        ProtocolEncoderV1::new(limits()).encode(&frame),
        Err(ProtocolError::UnsupportedSerializer { code: 2 })
    ));
    frame = ordinary();
    frame.compressor = CompressorType::Gzip;
    assert!(matches!(
        ProtocolEncoderV1::new(limits()).encode(&frame),
        Err(ProtocolError::UnsupportedCompressor { code: 1 })
    ));
    let mut bytes = ProtocolEncoderV1::new(limits())
        .encode(&ordinary())
        .unwrap();
    bytes[10] = 2;
    assert!(matches!(
        ProtocolDecoderV1::new(limits()).decode(&bytes),
        Err(ProtocolError::UnsupportedSerializer { code: 2 })
    ));
    bytes[10] = 1;
    bytes[11] = 1;
    assert!(matches!(
        ProtocolDecoderV1::new(limits()).decode(&bytes),
        Err(ProtocolError::UnsupportedCompressor { code: 1 })
    ));
}
#[test]
fn exact_frame_header_entry_and_payload_limits_apply_in_both_directions() {
    let mut frame = ordinary();
    frame.head_map = vec![("k".into(), None)];
    let bytes = ProtocolEncoderV1::new(limits()).encode(&frame).unwrap();
    let exact = FrameLimits::new(bytes.len(), 21, 1, limits().payload_limits()).unwrap();
    assert_eq!(ProtocolEncoderV1::new(exact).encode(&frame).unwrap(), bytes);
    assert_eq!(ProtocolDecoderV1::new(exact).decode(&bytes).unwrap(), frame);
    for limited in [
        FrameLimits::new(bytes.len() - 1, 21, 1, limits().payload_limits()).unwrap(),
        FrameLimits::new(bytes.len(), 20, 1, limits().payload_limits()).unwrap(),
        FrameLimits::new(bytes.len(), 21, 0, limits().payload_limits()).unwrap(),
        FrameLimits::new(bytes.len(), 21, 1, PayloadLimits::new(2, 0, 1, 0).unwrap()).unwrap(),
    ] {
        assert!(matches!(
            ProtocolEncoderV1::new(limited).encode(&frame),
            Err(ProtocolError::LimitExceeded { .. })
        ));
        assert!(matches!(
            ProtocolDecoderV1::new(limited).decode(&bytes),
            Err(ProtocolError::LimitExceeded { .. })
        ));
    }
    assert!(FrameLimits::new(15, 16, 1, limits().payload_limits()).is_err());
    assert!(FrameLimits::new(32768, 32768, 1, limits().payload_limits()).is_err());
    assert!(FrameLimits::new(i32::MAX as usize + 1, 16, 1, limits().payload_limits()).is_err());
}

#[test]
fn malformed_null_duplicate_utf8_and_unsealed_header_fields_are_rejected() {
    let body = ProtocolEncoderV1::new(limits())
        .encode(&ordinary())
        .unwrap()[16..]
        .to_vec();
    for header in [
        vec![255, 255, 0, 0],
        vec![255, 254, 0, 0],
        vec![0, 1, 255, 0, 0],
        vec![0, 1, b'k', 0, 0, 0, 1, b'k', 0, 0],
        vec![0, 0, 255, 254],
        vec![0],
        vec![0, 1, b'k'],
        vec![0, 1, b'k', 0, 2, b'x'],
        vec![127, 255],
        vec![0, 0, 0, 1, 255],
    ] {
        let mut bytes = vec![0xda, 0xda, 1];
        bytes.extend_from_slice(
            &u32::try_from(16 + header.len() + body.len())
                .unwrap()
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&i16::try_from(16 + header.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(&[0, 1, 0, 0, 0, 0, 1]);
        bytes.extend(header);
        bytes.extend(&body);
        assert!(ProtocolDecoderV1::new(limits()).decode(&bytes).is_err());
    }
}

#[test]
fn maximum_signed_header_boundary_and_metadata_only_debug_are_verified() {
    let mut frame = ordinary();
    frame.head_map = vec![("k".into(), Some("x".repeat(32746)))];
    let bytes = ProtocolEncoderV1::new(limits()).encode(&frame).unwrap();
    assert_eq!(&bytes[7..9], &32767i16.to_be_bytes());
    assert_eq!(
        ProtocolDecoderV1::new(limits()).decode(&bytes).unwrap(),
        frame
    );
    frame.head_map[0].1.as_mut().unwrap().push('x');
    assert!(matches!(
        ProtocolEncoderV1::new(limits()).encode(&frame),
        Err(ProtocolError::LimitExceeded { .. })
    ));
    frame.head_map = vec![("secret-key".into(), Some("secret-value".into()))];
    let debug = format!("{frame:?}");
    assert!(!debug.contains("secret-key") && !debug.contains("secret-value"));
}

#[test]
fn all_original_six_frames_are_reconstructed_from_their_certified_payloads() {
    for (name,kind,id,header,hex) in [
        ("GlobalBeginRequest",RequestType::RequestSync,1,vec![],"dada010000002a0010000100000000010001000007d000125365617461476f5472616e73616374696f6e"),
        ("GlobalBeginResponse",RequestType::Response,1,vec![],"dada01000000280010010100000000010002010000103132372e302e302e313a383039313a310000"),
        ("GlobalBeginRequest",RequestType::RequestSync,1,vec![("traceId".into(),Some("trace-001".into()))],"dada010000003e002400010000000001000774726163654964000974726163652d3030310001000007d000125365617461476f5472616e73616374696f6e"),
        ("GlobalBeginResponseFailed",RequestType::Response,1,vec![],"dada0100000026001001010000000001000200000c626567696e2064656e6965640100000000"),
        ("RegisterTMRequest",RequestType::RequestSync,2,vec![],"dada01000000390010000100000000020065000456312c30000b6f72646572732d72757374001064656661756c745f74785f67726f75700000"),
        ("RegisterRMRequest",RequestType::RequestSync,3,vec![],"dada010000004b0010000100000000030067000456312c30000b6f72646572732d72757374001064656661756c745f74785f67726f757000000000000e6d7973716c3a2f2f6f7264657273"),
    ] {
        let payload=std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/protocol/v1").join(format!("{name}.bin"))).unwrap();
        let frame=RpcMessage {id,message_type:kind,codec:SerializerType::Seata,compressor:CompressorType::None,
            head_map:header,body:Some(seata_protocol::PayloadCodec::with_limits(limits().payload_limits()).decode(&payload).unwrap())};
        assert_eq!(ProtocolEncoderV1::new(limits()).encode(&frame).unwrap(),unhex(hex));
        assert_eq!(ProtocolDecoderV1::new(limits()).decode(&unhex(hex)).unwrap(),frame);
    }
}

#[test]
fn accepted_single_byte_frame_mutations_roundtrip_exactly() {
    let mut frame = ordinary();
    frame.head_map = vec![("k".into(), None)];
    let bytes = ProtocolEncoderV1::new(limits()).encode(&frame).unwrap();
    let mut accepted = 0;
    for i in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[i] ^= 0x80;
        if let Ok(value) = ProtocolDecoderV1::new(limits()).decode(&changed) {
            assert_eq!(
                ProtocolEncoderV1::new(limits()).encode(&value).unwrap(),
                changed
            );
            accepted += 1;
        }
    }
    assert!(accepted > 0);
}

#[test]
fn header_byte_budget_precedes_hashing_and_duplicate_key_validation() {
    let mut frame = ordinary();
    frame.head_map = vec![("k".into(), None), ("k".into(), None)];
    let restricted = FrameLimits::new(1024, 21, 2, limits().payload_limits()).unwrap();
    assert!(matches!(
        ProtocolEncoderV1::new(restricted).encode(&frame),
        Err(ProtocolError::LimitExceeded { .. })
    ));
}

#[test]
fn all_37_fresh_original_java_frames_match_formal_objects_and_encoding() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/protocol/v1/formal_frames/java.tsv"),
    )
    .unwrap();
    let mut names = std::collections::BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let cols: Vec<_> = line.split('\t').collect();
        assert_eq!(cols.len(), 6);
        let name = cols[0];
        assert!(names.insert(name));
        let mut expected = ordinary();
        if index < 29 {
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/protocol/v1")
                    .join(format!("{name}.bin")),
            )
            .unwrap();
            let body = seata_protocol::PayloadCodec::with_limits(limits().payload_limits())
                .decode(&bytes)
                .unwrap();
            expected.message_type = if body.is_result() {
                RequestType::Response
            } else {
                RequestType::RequestSync
            };
            expected.body = Some(body);
            expected.id = i32::try_from(index + 1).unwrap();
            assert_eq!(unhex(cols[5]), bytes);
        } else {
            match name {
                "HeaderNull" => expected.head_map = vec![("k".into(), None)],
                "HeaderEmpty" => expected.head_map = vec![("k".into(), Some("".into()))],
                "HeaderEmptyKey" => expected.head_map = vec![("".into(), Some("".into()))],
                "HeaderOrdered" => {
                    expected.head_map = vec![
                        ("z".into(), None),
                        ("".into(), Some("".into())),
                        ("a".into(), Some("中文🦀".into())),
                    ]
                }
                "HeaderMaximum" => expected.head_map = vec![("k".into(), Some("x".repeat(32746)))],
                "SignedMinimum" => expected.id = i32::MIN,
                "SignedMaximum" => expected.id = i32::MAX,
                "Oneway" => expected.message_type = RequestType::RequestOneway,
                _ => panic!("unexpected Java case"),
            }
        }
        assert_eq!(cols[1].parse::<i32>().unwrap(), expected.id);
        assert_eq!(
            cols[2].parse::<u8>().unwrap(),
            expected.message_type.to_java_v1()
        );
        let bytes = unhex(cols[4]);
        let head = usize::from(u16::from_be_bytes([bytes[7], bytes[8]]));
        assert_eq!(&bytes[16..head], unhex(cols[3]));
        assert_eq!(
            ProtocolDecoderV1::new(limits()).decode(&bytes).unwrap(),
            expected,
            "{name}"
        );
        assert_eq!(
            ProtocolEncoderV1::new(limits()).encode(&expected).unwrap(),
            bytes,
            "{name}"
        );
        for n in 0..bytes.len() {
            assert!(
                ProtocolDecoderV1::new(limits())
                    .decode(&bytes[..n])
                    .is_err(),
                "{name}/{n}"
            );
        }
    }
    assert_eq!(names.len(), 37);
    for name in [
        "HeaderNull",
        "HeaderEmpty",
        "HeaderEmptyKey",
        "HeaderOrdered",
        "HeaderMaximum",
        "SignedMinimum",
        "SignedMaximum",
        "Oneway",
    ] {
        assert!(names.contains(name));
    }
}
