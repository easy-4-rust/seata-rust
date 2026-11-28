//! 单表别名不能改变真实资源身份、参数次序或拒绝边界。
#![allow(clippy::unwrap_used)]
use seata_at::SqlRecognizerFactory;

#[test]
fn single_table_alias_and_qualified_columns_preserve_bindings_and_identity() {
    for (sql, alias) in [
        (
            "UPDATE accounts AS a SET a.balance=a.balance-?, note=? WHERE a.id IN (?,?)",
            Some("a"),
        ),
        (
            "UPDATE `accounts` `a` SET `a`.`balance`=7 WHERE `a`.`id`=?",
            Some("a"),
        ),
        ("DELETE FROM accounts AS a WHERE a.id=?", Some("a")),
        ("DELETE FROM accounts a WHERE a.id=?", Some("a")),
        (
            "UPDATE accounts SET accounts.balance=accounts.balance-? WHERE accounts.id=?",
            None,
        ),
    ] {
        let plan = SqlRecognizerFactory::recognize(sql).unwrap();
        assert_eq!(plan.table_name(), "accounts");
        assert_eq!(plan.table_alias(), alias);
        assert_eq!(plan.sql(), sql);
    }
    let plan = SqlRecognizerFactory::recognize(
        "UPDATE accounts AS a SET a.balance=a.balance-?, note=? WHERE a.id IN (?,?)",
    )
    .unwrap();
    assert_eq!(plan.columns(), &["balance", "note"]);
    assert_eq!(plan.parameter_count(), 4);
    assert_eq!(plan.predicate_parameter_start(), 2);
    assert_eq!(plan.predicate_sql(), Some("a.id IN (?, ?)"));
}

#[test]
fn unrelated_qualifiers_duplicate_targets_and_alias_extensions_stay_rejected() {
    for sql in [
        "UPDATE accounts a SET b.balance=1",
        "UPDATE accounts a SET balance=b.balance",
        "UPDATE accounts a SET balance=1 WHERE b.id=1",
        "UPDATE accounts a SET balance=1 WHERE accounts.id=1",
        "UPDATE accounts a SET balance=1,a.BALANCE=2",
        "UPDATE accounts SET balance=1 WHERE other.id=1",
        "UPDATE accounts a SET balance=db.a.balance",
        "UPDATE accounts AS a(id) SET balance=1",
        "DELETE a FROM accounts a JOIN other b ON a.id=b.id",
        "UPDATE accounts a JOIN other b ON a.id=b.id SET a.balance=1",
    ] {
        assert!(
            SqlRecognizerFactory::recognize(sql).is_err(),
            "accepted {sql}"
        );
    }
}
