//! 实际编译固定 Java 对象所得字节向量；无手写替代编码器。
#![allow(clippy::unwrap_used)]
use seata_xa::{XaBranchXid, XaXid};
#[test]
fn original_java_vectors_match_every_component() {
    fn decode(text: &str) -> Vec<u8> {
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }
    let mut count = 0;
    for line in include_str!("fixtures/java_xid.tsv").lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 5);
        let global = decode(fields[3]);
        let branch = decode(fields[4]);
        let xid = if fields[0] == "0" {
            None
        } else {
            Some(String::from_utf8(global.clone()).unwrap())
        };
        let value = XaBranchXid::new(xid, fields[1].parse().unwrap());
        assert_eq!(value.get_format_id().to_string(), fields[2]);
        assert_eq!(value.get_global_transaction_id(), global);
        assert_eq!(
            value.get_branch_qualifier(),
            branch,
            "fixed Java XA byte identity changed"
        );
        count += 1;
    }
    assert_eq!(count, 8);
}
