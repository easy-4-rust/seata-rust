//! 原 Java 编码语义与数据库严格入口分开验证。
#![allow(clippy::unwrap_used)]
use seata_xa::{XaBranchXid, XaError, XaXid};

#[test]
fn java_branch_prefix_and_signed_long_are_preserved() {
    for branch in [1, 17, i64::MAX, i64::MIN, -17] {
        let value = XaBranchXid::new(Some("tc:中".into()), branch);
        assert_eq!(
            value.get_branch_qualifier(),
            format!("-{branch}").as_bytes(),
            "original Java XA branch prefix differs"
        );
        assert_eq!(value.get_global_transaction_id(), "tc:中".as_bytes());
        assert_eq!(value.get_format_id(), 9752);
        assert_eq!(
            XaBranchXid::from_components(
                9752,
                value.get_global_transaction_id(),
                value.get_branch_qualifier()
            )
            .unwrap(),
            value
        );
    }
}
#[test]
fn java_null_empty_and_zero_remain_distinct_logical_values() {
    let null = XaBranchXid::new(None, 0);
    let empty = XaBranchXid::new(Some(String::new()), 0);
    assert_ne!(null, empty);
    assert_eq!(null.get_global_xid(), None);
    assert_eq!(empty.get_global_xid(), Some(""));
    assert!(null.get_global_transaction_id().is_empty());
    assert!(null.get_branch_qualifier().is_empty());
    assert_eq!(XaBranchXid::from_components(9752, b"", b"").unwrap(), null);
    assert!(null.mysql_arguments().is_err());
    assert!(empty.mysql_arguments().is_err());
}
#[test]
fn mysql_identity_is_bounded_and_uses_only_original_hex() {
    let value = XaBranchXid::new(Some("a',';\\\n中".into()), 17);
    let sql = value.mysql_arguments().unwrap();
    assert!(!sql.contains("a',"));
    assert!(sql.ends_with(",X'2D3137',9752"));
    assert!(XaBranchXid::new(Some("中".repeat(21) + "a"), 1)
        .mysql_arguments()
        .is_ok());
    assert!(XaBranchXid::new(Some("中".repeat(22)), 1)
        .mysql_arguments()
        .is_err());
    assert!(XaBranchXid::new(Some("x".into()), -1)
        .mysql_arguments()
        .is_err());
    assert!(!format!("{value:?}").contains("中"));
}
#[test]
fn malformed_or_noncanonical_recovery_bytes_are_rejected() {
    for (global, branch) in [
        (b"x".as_slice(), b"-01".as_slice()),
        (b"x", b"-0"),
        (b"x", b"17"),
        (b"x", b"-+1"),
        (b"\xff", b"-1"),
        (b"x", b"--9223372036854775809"),
    ] {
        assert_eq!(
            XaBranchXid::from_components(9752, global, branch),
            Err(XaError::Components)
        );
    }
    assert_eq!(
        XaBranchXid::from_components(1, b"x", b"-1"),
        Err(XaError::Format)
    );
}
