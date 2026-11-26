//! Rust表示、原测试未覆盖分支及完整枚举/常量集合的补充契约。
use seata_sqlparser::{
    EscapeSymbol, JdbcConstants, SQLParsingException, SQLType, SqlMethodExpr, SqlParserType,
    SqlSequenceExpr,
};
use std::error::Error;

#[test]
fn escape_preserves_utf16_code_units() {
    let symbol = EscapeSymbol::new(0xd800);
    assert_eq!(symbol.left_symbol(), 0xd800);
    assert_eq!(symbol.right_symbol(), 0xd800);
    let paired = EscapeSymbol::paired(91, 93);
    assert_eq!((paired.left_symbol(), paired.right_symbol()), (91, 93));
}

#[test]
fn method_count_is_signed_and_participates_in_equality() {
    for count in [i32::MIN, -1, 0, 1, i32::MAX] {
        let expr = SqlMethodExpr::new(count);
        assert_eq!(expr.placeholder_count(), count);
        assert_eq!(expr.java_hash_code(), count);
        assert_eq!(expr, SqlMethodExpr::new(count));
        assert_eq!(expr.to_string(), "SQL_METHOD");
    }
    assert_ne!(SqlMethodExpr::new(1), SqlMethodExpr::new(2));
    assert_eq!(SqlMethodExpr::get().placeholder_count(), 0);
    assert!(std::ptr::eq(SqlMethodExpr::get(), SqlMethodExpr::get()));
}

#[test]
fn sequence_empty_and_null_remain_distinct() {
    let mut expr = SqlSequenceExpr::new(Some(String::new()), Some(String::new()));
    assert_eq!(expr.sequence(), Some(""));
    assert_eq!(expr.function(), Some(""));
    expr.set_sequence(None);
    expr.set_function(None);
    assert_eq!(expr.sequence(), None);
    assert_eq!(expr.function(), None);
}

#[test]
fn exception_null_and_empty_messages_remain_distinct() {
    let absent = SQLParsingException::new(None);
    assert_eq!(absent.message(), None);
    assert!(absent.source().is_none());
    assert_eq!(
        absent.to_string(),
        "org.apache.seata.sqlparser.SQLParsingException"
    );
    let empty = SQLParsingException::new(Some(String::new()));
    assert_eq!(empty.message(), Some(""));
    assert_eq!(
        empty.to_string(),
        "org.apache.seata.sqlparser.SQLParsingException: "
    );
    let null_cause = SQLParsingException::from_cause(None);
    assert_eq!(null_cause.message(), None);
    assert!(null_cause.cause().is_none());
    assert!(null_cause.source().is_none());
}

#[test]
fn all_sql_type_values_round_trip_and_holes_are_rejected() -> Result<(), Box<dyn Error>> {
    let numbers: Vec<i32> = (0..=44).chain(101..=103).collect();
    assert_eq!(SQLType::ALL.len(), 48);
    for (value, expected) in SQLType::ALL.into_iter().zip(numbers) {
        assert_eq!(value.value(), expected);
        assert_eq!(SQLType::try_from(expected)?, value);
    }
    for invalid in [i32::MIN, -1, 45, 100, 104, i32::MAX] {
        let error = SQLType::try_from(invalid)
            .err()
            .ok_or("Enum hole accepted")?;
        assert_eq!(error.value(), invalid);
        assert_eq!(error.to_string(), format!("Invalid SQLType:{invalid}"));
    }
    Ok(())
}

#[test]
fn parser_and_jdbc_constant_contracts() {
    assert_eq!(SqlParserType::SQL_PARSER_TYPE_DRUID, "druid");
    assert_eq!(SqlParserType::SQL_PARSER_TYPE_ANTLR, "antlr");
    assert_eq!(JdbcConstants::OCEANBASE_ORACLE, "oceanbase_oracle");
    assert_eq!(JdbcConstants::POLARDBX, "polardb-x");
    assert_eq!(JdbcConstants::MYSQL, "mysql");
    assert_eq!(JdbcConstants::SQLSERVER, "sqlserver");
}
