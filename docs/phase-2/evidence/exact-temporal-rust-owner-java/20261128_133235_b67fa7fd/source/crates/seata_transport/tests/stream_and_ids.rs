//! WP02分片/连续帧及单连接编号所有权；不宣称已实现TCP或pending futures。
#![allow(clippy::unwrap_used)]
use seata_protocol::protocol::RpcMessage;
use seata_protocol::v1::{FrameLimits, ProtocolDecoderV1};
use seata_protocol::PayloadLimits;
use seata_transport::{FrameDecoder, RequestIdAllocator, TransportError};
use std::collections::HashSet;

fn limits() -> FrameLimits {
    FrameLimits::new(
        64 * 1024,
        32767,
        128,
        PayloadLimits::new(64 * 1024, 512, 1024, 16).unwrap(),
    )
    .unwrap()
}
fn frames() -> Vec<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/protocol/v1/control_envelopes");
    [
        "HeartbeatPing",
        "HeartbeatPong",
        "HeartbeatHeadMap",
        "MergeRequestWithIds",
        "MergeRequestWithoutIds",
        "MergeRequestEmpty",
        "MergeRequestNested",
        "MergeResult",
        "MergeResultEmpty",
    ]
    .into_iter()
    .map(|name| std::fs::read(root.join(format!("{name}.frame.bin"))).unwrap())
    .collect()
}
fn read_all(decoder: &mut FrameDecoder, mut input: &[u8]) -> Vec<RpcMessage> {
    let mut output = vec![];
    while !input.is_empty() {
        let before = input.len();
        match decoder.decode(&mut input).unwrap() {
            Some(value) => output.push(value),
            None => assert!(input.is_empty()),
        }
        assert!(input.len() < before, "stream decoder must make progress");
    }
    output
}
#[test]
fn every_java_control_frame_handles_every_two_way_split_and_one_byte_fragments() {
    for frame in frames() {
        let expected = ProtocolDecoderV1::new(limits()).decode(&frame).unwrap();
        for split in 0..=frame.len() {
            let mut decoder = FrameDecoder::new(limits());
            let mut got = read_all(&mut decoder, &frame[..split]);
            got.extend(read_all(&mut decoder, &frame[split..]));
            assert_eq!(got, vec![expected.clone()]);
            assert_eq!(decoder.buffered_bytes(), 0);
            decoder.finish().unwrap();
        }
        let mut decoder = FrameDecoder::new(limits());
        let mut got = vec![];
        for byte in &frame {
            got.extend(read_all(&mut decoder, std::slice::from_ref(byte)));
        }
        assert_eq!(got, vec![expected]);
        decoder.finish().unwrap();
    }
}
#[test]
fn coalesced_input_larger_than_per_frame_budget_is_drained_one_frame_at_a_time() {
    let frame = frames()[0].clone();
    let compact = FrameLimits::new(16, 16, 0, PayloadLimits::new(2, 0, 1, 0).unwrap()).unwrap();
    let mut decoder = FrameDecoder::new(compact);
    let bytes = frame.repeat(1000);
    let mut input = bytes.as_slice();
    for left in (0..1000).rev() {
        assert!(decoder.decode(&mut input).unwrap().is_some());
        assert_eq!(input.len(), left * 16);
        assert_eq!(decoder.buffered_bytes(), 0);
        assert!(decoder.reserved_bytes() <= 16);
    }
    assert!(decoder.decode(&mut input).unwrap().is_none());
    decoder.finish().unwrap();
    assert!(matches!(
        decoder.decode(&mut input),
        Err(TransportError::DecoderClosed)
    ));
}
#[test]
fn coalesced_good_frame_is_delivered_before_following_corrupt_frame_terminates_stream() {
    let good = frames()[0].clone();
    let mut bad = good.clone();
    bad[0] = 0;
    let combined = [good, bad].concat();
    let mut input = combined.as_slice();
    let mut decoder = FrameDecoder::new(limits());
    assert!(decoder.decode(&mut input).unwrap().is_some());
    assert_eq!(input.len(), 16);
    assert!(matches!(
        decoder.decode(&mut input),
        Err(TransportError::Protocol(_))
    ));
    assert!(decoder.is_closed());
    assert!(matches!(
        decoder.decode(&mut input),
        Err(TransportError::DecoderClosed)
    ));
}
#[test]
fn eof_with_partial_header_or_body_is_an_error_and_never_reuses_the_decoder() {
    let frame = frames()[3].clone();
    for length in 1..frame.len() {
        let mut decoder = FrameDecoder::new(limits());
        let mut input = &frame[..length];
        assert!(decoder.decode(&mut input).unwrap().is_none());
        assert!(input.is_empty());
        assert!(
            matches!(decoder.finish(),Err(TransportError::UnexpectedEof{buffered,..}) if buffered==length)
        );
        assert!(decoder.is_closed());
        let mut remaining = &frame[length..];
        assert!(matches!(
            decoder.decode(&mut remaining),
            Err(TransportError::DecoderClosed)
        ));
    }
}
#[test]
fn prefix_validation_rejects_hostile_lengths_and_capabilities_before_body_buffering() {
    let frame = frames()[3].clone();
    let p = ProtocolDecoderV1::new(limits());
    for n in 0..16 {
        assert_eq!(p.frame_length(&frame[..n]).unwrap(), None);
    }
    assert_eq!(p.frame_length(&frame[..16]).unwrap(), Some(frame.len()));
    let mut big = frame.clone();
    big[3..7].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(p.frame_length(&big[..7]).is_err());
    let mut d = FrameDecoder::new(limits());
    let mut input = &big[..7];
    assert!(matches!(
        d.decode(&mut input),
        Err(TransportError::Protocol(_))
    ));
    assert!(d.buffered_bytes() <= 16);
    assert!(d.reserved_bytes() <= 16);
    assert!(d.is_closed());
    for (index, value) in [
        (0, 0),
        (1, 0),
        (2, 2),
        (9, 5),
        (10, 2),
        (10, 255),
        (11, 1),
        (11, 255),
    ] {
        let mut bad = frame.clone();
        bad[index] = value;
        assert!(p.frame_length(&bad[..index + 1]).is_err(), "index {index}");
        let mut decoder = FrameDecoder::new(limits());
        let mut input = bad.as_slice();
        assert!(matches!(
            decoder.decode(&mut input),
            Err(TransportError::Protocol(_))
        ));
        assert!(decoder.buffered_bytes() <= 16);
        assert!(decoder.reserved_bytes() <= 16);
    }
}
#[test]
fn empty_fragments_do_not_close_or_consume_valid_partial_state_and_debug_hides_payload() {
    let frame = frames()[3].clone();
    let mut decoder = FrameDecoder::new(limits());
    let mut part = &frame[..20];
    assert!(decoder.decode(&mut part).unwrap().is_none());
    let mut empty = &[][..];
    assert!(decoder.decode(&mut empty).unwrap().is_none());
    assert!(!decoder.is_closed());
    assert_eq!(decoder.buffered_bytes(), 20);
    let debug = format!("{decoder:?}");
    assert!(!debug.contains("m-a") && !debug.contains("application_data"));
    let mut rest = &frame[20..];
    assert!(decoder.decode(&mut rest).unwrap().is_some());
    decoder.finish().unwrap();
}
#[test]
fn identifier_sequence_preserves_signed_wrap_and_zero_without_reuse() {
    let mut ids = RequestIdAllocator::new(i32::MAX - 1, 5).unwrap();
    for value in [i32::MAX - 1, i32::MAX, i32::MIN, i32::MIN + 1, i32::MIN + 2] {
        assert_eq!(ids.allocate().unwrap(), value);
    }
    assert_eq!(ids.issued(), 5);
    assert_eq!(ids.remaining(), 0);
    for _ in 0..2 {
        assert!(matches!(
            ids.allocate(),
            Err(TransportError::RequestIdRotationRequired {
                issued: 5,
                maximum: 5
            })
        ));
    }
    let mut ids = RequestIdAllocator::new(-1, 3).unwrap();
    assert_eq!(
        [
            ids.allocate().unwrap(),
            ids.allocate().unwrap(),
            ids.allocate().unwrap()
        ],
        [-1, 0, 1]
    );
}
#[test]
fn allocator_quota_requires_rotation_and_all_32_bits_can_be_configured() {
    for maximum in [0, (1u64 << 32) + 1, u64::MAX] {
        assert!(RequestIdAllocator::new(1, maximum).is_err());
    }
    let ids = RequestIdAllocator::new(1, 1u64 << 32).unwrap();
    assert_eq!(ids.remaining(), 1u64 << 32);
    for start in [i32::MIN, -100, -1, 0, 1, 100, i32::MAX] {
        let mut ids = RequestIdAllocator::new(start, 4096).unwrap();
        let mut used = HashSet::new();
        for _ in 0..4096 {
            assert!(used.insert(ids.allocate().unwrap()));
        }
        assert!(ids.allocate().is_err());
    }
}

fn unhex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn all_37_actual_java_frames_include_large_nullable_headers_and_every_payload_family() {
    let rows = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/protocol/v1/formal_frames/java.tsv"),
    )
    .unwrap();
    let mut seen = HashSet::new();
    for row in rows.lines() {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 6);
        assert!(seen.insert(fields[0]));
        let frame = unhex(fields[4]);
        let expected = ProtocolDecoderV1::new(limits()).decode(&frame).unwrap();
        let head = usize::from(u16::from_be_bytes([frame[7], frame[8]]));
        let mut splits = vec![
            0,
            1,
            2,
            3,
            7,
            9,
            10,
            11,
            12,
            15,
            16,
            head - 1,
            head,
            head + 1,
            frame.len() - 1,
            frame.len(),
        ];
        splits.sort_unstable();
        splits.dedup();
        for split in splits.into_iter().filter(|n| *n <= frame.len()) {
            let mut decoder = FrameDecoder::new(limits());
            let mut got = read_all(&mut decoder, &frame[..split]);
            got.extend(read_all(&mut decoder, &frame[split..]));
            assert_eq!(got, vec![expected.clone()], "{} split {split}", fields[0]);
            decoder.finish().unwrap();
        }
        let mut decoder = FrameDecoder::new(limits());
        let mut got = vec![];
        for byte in &frame {
            got.extend(read_all(&mut decoder, std::slice::from_ref(byte)));
            assert!(decoder.buffered_bytes() <= frame.len());
            assert!(decoder.reserved_bytes() <= limits().maximum_frame_length());
        }
        assert_eq!(got, vec![expected]);
        decoder.finish().unwrap();
    }
    assert_eq!(seen.len(), 37);
}

#[test]
fn partial_fixed_header_rejects_signed_head_and_body_shape_without_waiting_for_payload() {
    let good = frames()[3].clone();
    let protocol = ProtocolDecoderV1::new(limits());
    for length in [0u16, 15, 32768, 65535] {
        let mut bad = good.clone();
        bad[7..9].copy_from_slice(&length.to_be_bytes());
        assert!(protocol.frame_length(&bad[..9]).is_err());
    }
    let mut ordinary_empty = frames()[0].clone();
    ordinary_empty[9] = 0;
    assert!(protocol.frame_length(&ordinary_empty[..16]).is_err());
    let mut heartbeat_body = good;
    heartbeat_body[9] = 3;
    assert!(protocol.frame_length(&heartbeat_body[..16]).is_err());
    // 前缀探测忽略完整帧后的连续数据；完整帧API仍严格拒绝同一输入中的后缀。
    let joined = frames()[0].repeat(5000);
    assert_eq!(protocol.frame_length(&joined).unwrap(), Some(16));
    assert!(protocol.decode(&joined).is_err());
}
