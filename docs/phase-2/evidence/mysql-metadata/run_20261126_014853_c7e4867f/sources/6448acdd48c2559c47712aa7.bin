//! 固定 Fastjson 原 H2 输入，经 Rust 编解码执行原断言。
#![allow(clippy::unwrap_used)]
#[path = "undo_h2_input.rs"]
mod input;
use input::from_h2;
use seata_at::FastjsonUndoLogParser;

#[test]
#[ignore = "requires fixed source JDBC H2 input trace"]
fn fastjson_source_test_encode_and_decode() {
    let (xid, id, logs) = from_h2("testEncodeAndDecode");
    let parser = FastjsonUndoLogParser::default();
    let bytes = parser.encode_branch(Some(&xid), id, Some(&logs)).unwrap();
    assert!(!bytes.is_empty());
    let branch = parser.decode_branch(&bytes).unwrap().unwrap();
    assert_eq!(id, branch.branch_id());
    assert_eq!(Some(xid.as_str()), branch.xid());
    let decoded = branch.decode_sql_undo_logs().unwrap().unwrap();
    assert_eq!(logs.len(), decoded.len());
    for (expected, actual) in logs.iter().zip(&decoded) {
        assert_eq!(expected.sql_type(), actual.sql_type());
        assert_eq!(expected.table_name(), actual.table_name());
        assert!(expected.before_image() == actual.before_image());
        assert!(expected.after_image() == actual.after_image());
    }
    let mut observations = vec![
        (!bytes.is_empty()).to_string(),
        branch.branch_id().to_string(),
        branch.xid().unwrap().into(),
        decoded.len().to_string(),
    ];
    for (expected, actual) in logs.iter().zip(&decoded) {
        observations.extend([
            format!("{:?}", actual.sql_type()).to_ascii_uppercase(),
            actual.table_name().unwrap().into(),
            (expected.before_image() == actual.before_image()).to_string(),
            (expected.after_image() == actual.after_image()).to_string(),
        ]);
    }
    observations.push(String::from_utf8(bytes).unwrap());
    println!("SEATA_FASTJSON_H2_ROUNDTRIP\t{}", observations.join("\t"));
}

#[test]
#[ignore = "requires fixed H2 trace and original 100000 codec iterations"]
fn fastjson_source_test_performance() {
    let (xid, id, logs) = from_h2("testPerformance");
    let parser = FastjsonUndoLogParser::default();
    let mut observed = 0;
    let mut last = Vec::new();
    for _ in 0..100_000 {
        last = parser.encode_branch(Some(&xid), id, Some(&logs)).unwrap();
        assert!(!last.is_empty());
        let branch = parser.decode_branch(&last).unwrap().unwrap();
        assert_eq!(branch.branch_id(), id);
        std::hint::black_box(branch.decode_sql_undo_logs().unwrap().unwrap());
        observed += 1;
    }
    println!(
        "SEATA_FASTJSON_H2_PERFORMANCE\t{observed}\ttrue\t{id}\t{}",
        std::str::from_utf8(&last).unwrap()
    );
}
