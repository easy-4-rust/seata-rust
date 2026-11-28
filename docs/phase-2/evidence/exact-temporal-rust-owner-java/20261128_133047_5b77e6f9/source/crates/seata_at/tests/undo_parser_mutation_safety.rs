//! 跨格式 Undo 解码负向语料：截断和单字节损坏不能使解析器崩溃。
#![allow(clippy::unwrap_used)]

use seata_at::UndoLogParser;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn decode(parser: &UndoLogParser, bytes: &[u8]) -> bool {
    match parser {
        UndoLogParser::Fastjson(codec) => codec.decode_branch(bytes).is_ok(),
        UndoLogParser::Jackson(codec) => codec.decode(bytes).is_ok(),
        UndoLogParser::Protostuff(codec) => codec.decode_branch(bytes).is_ok(),
        UndoLogParser::Kryo(codec) => codec.decode_branch(bytes).is_ok(),
        UndoLogParser::Fastjson2(codec) => codec.decode_branch(bytes).is_ok(),
        UndoLogParser::Fory(codec) => codec.decode_branch(bytes).is_ok(),
    }
}

fn nonempty_branch(parser: &UndoLogParser) -> Vec<u8> {
    match parser {
        UndoLogParser::Fastjson(codec) => {
            codec.encode_branch(Some("mutation:1"), 17, None).unwrap()
        }
        UndoLogParser::Jackson(codec) => {
            codec.encode_original(Some("mutation:1"), 17, None).unwrap()
        }
        UndoLogParser::Protostuff(codec) => {
            codec.encode_branch(Some("mutation:1"), 17, None).unwrap()
        }
        UndoLogParser::Kryo(codec) => codec.encode_branch(Some("mutation:1"), 17, None).unwrap(),
        UndoLogParser::Fastjson2(codec) => {
            codec.encode_branch(Some("mutation:1"), 17, None).unwrap()
        }
        UndoLogParser::Fory(codec) => codec.encode_branch(Some("mutation:1"), 17, None).unwrap(),
    }
}

fn exercise(name: &str, parser: &UndoLogParser, seed: &[u8]) -> usize {
    assert!(decode(parser, seed), "{name}: seed must decode");
    let mut cases = 0;
    let mut check = |candidate: &[u8]| {
        assert!(
            catch_unwind(AssertUnwindSafe(|| decode(parser, candidate))).is_ok(),
            "{name}: parser panicked on {candidate:02x?}"
        );
        cases += 1;
    };

    // 短报文逐字节截断；长报文也覆盖末尾与均匀分布的切点。
    for cut in 0..=seed.len().min(16) {
        check(&seed[..cut]);
    }
    for delta in 0..=seed.len().min(16) {
        check(&seed[..seed.len() - delta]);
    }
    for slot in 0..128 {
        let index = slot * seed.len() / 128;
        if index >= seed.len() {
            continue;
        }
        for replacement in [0, 0xff, seed[index] ^ 0x80] {
            let mut candidate = seed.to_vec();
            candidate[index] = replacement;
            check(&candidate);
        }
    }
    cases
}

#[test]
fn all_six_decoders_survive_bounded_deterministic_corruption() {
    let mut cases = 0;
    for name in [
        "fastjson",
        "jackson",
        "protostuff",
        "kryo",
        "fastjson2",
        "fory",
    ] {
        let parser = UndoLogParser::load(Some(name)).unwrap();
        cases += exercise(name, parser, &parser.default_content().unwrap());
        cases += exercise(name, parser, &nonempty_branch(parser));
        if name == "fastjson" {
            cases += exercise(
                name,
                parser,
                include_bytes!("fixtures/fastjson_native_wire.txt"),
            );
        } else if name == "jackson" {
            cases += exercise(
                name,
                parser,
                include_bytes!("fixtures/branch_undo_java.json"),
            );
        } else if name == "fastjson2" {
            cases += exercise(
                name,
                parser,
                include_bytes!("fixtures/fastjson2_native_wire.bin"),
            );
        }
    }
    assert!(
        cases >= 1_000,
        "mutation corpus unexpectedly small: {cases}"
    );
}
