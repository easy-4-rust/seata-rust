//! MySQL AT支持/拒绝矩阵及解析器替代带来的完整输入、预算与绑定风险。
#![allow(clippy::unwrap_used)]
use seata_at::{SqlError, SqlRecognizerFactory, SqlType};

#[test]
fn insert_retains_table_columns_rows_original_sql_and_real_parameters() {
    let sql = "INSERT INTO `Account` (`id`,balance,note) VALUES (?, ?, '?'), (?, 3, 'semi;colon')";
    let plan = SqlRecognizerFactory::recognize(sql).unwrap();
    assert_eq!(plan.sql_type(), SqlType::Insert);
    assert_eq!(plan.sql_type().java_code(), 1);
    assert_eq!(plan.table_name(), "Account");
    assert_eq!(plan.columns(), &["id", "balance", "note"]);
    assert_eq!(plan.insert_row_count(), 2);
    assert_eq!(plan.parameter_count(), 3);
    assert_eq!(plan.predicate_parameter_start(), 3);
    assert_eq!(plan.predicate_sql(), None);
    assert_eq!(plan.sql(), sql);
}

#[test]
fn update_separates_assignment_parameters_from_where_image_parameters() {
    let plan = SqlRecognizerFactory::recognize(
        "UPDATE accounts SET balance = balance - ?, note = ? WHERE id IN (?, ?) AND deleted IS NULL",
    ).unwrap();
    assert_eq!(plan.sql_type(), SqlType::Update);
    assert_eq!(plan.sql_type().java_code(), 2);
    assert_eq!(plan.columns(), &["balance", "note"]);
    assert_eq!(plan.parameter_count(), 4);
    assert_eq!(plan.predicate_parameter_start(), 2);
    assert_eq!(
        plan.predicate_sql(),
        Some("id IN (?, ?) AND deleted IS NULL")
    );
    assert_eq!(plan.insert_row_count(), 0);
}

#[test]
fn delete_supports_bounded_predicates_and_absent_where_without_inventing_one() {
    let plan = SqlRecognizerFactory::recognize(
        "DELETE FROM accounts WHERE (id BETWEEN ? AND ?) OR note IS NOT NULL;",
    )
    .unwrap();
    assert_eq!(plan.sql_type(), SqlType::Delete);
    assert_eq!(plan.sql_type().java_code(), 3);
    assert_eq!(plan.parameter_count(), 2);
    assert_eq!(plan.predicate_parameter_start(), 0);
    assert!(plan.columns().is_empty());
    assert_eq!(
        SqlRecognizerFactory::recognize("delete from accounts")
            .unwrap()
            .predicate_sql(),
        None
    );
}

#[test]
fn rejects_entire_unsupported_statement_before_exposing_an_executable_plan() {
    for sql in [
        "SELECT * FROM accounts",
        "BEGIN",
        "COMMIT",
        "ROLLBACK",
        "TRUNCATE accounts",
        "ALTER TABLE accounts ADD balance INT",
        "CREATE TABLE accounts (id INT)",
        "CALL debit(1)",
        "REPLACE INTO accounts(id) VALUES(1)",
        "INSERT IGNORE INTO accounts(id) VALUES(1)",
        "INSERT INTO accounts(id) VALUES(1) ON DUPLICATE KEY UPDATE id=2",
        "INSERT INTO accounts(id) SELECT id FROM other",
        "INSERT INTO accounts VALUES(1)",
        "INSERT INTO accounts(id) VALUES(1) RETURNING id",
        "INSERT INTO accounts(id) VALUES ROW(1)",
        "INSERT INTO accounts(id) VALUES(DEFAULT)",
        "UPDATE accounts a SET a.balance=1",
        "UPDATE accounts JOIN other ON accounts.id=other.id SET balance=1",
        "DELETE accounts FROM accounts JOIN other ON accounts.id=other.id",
        "DELETE FROM accounts ORDER BY id LIMIT 1",
        "UPDATE db.accounts SET balance=1",
        "UPDATE accounts SET balance=(SELECT balance FROM other)",
        "WITH x AS (SELECT 1) UPDATE accounts SET balance=1",
        "UPDATE accounts SET balance=RAND()",
        "UPDATE accounts SET balance=@v",
        "UPDATE accounts SET balance=1 WHERE EXISTS (SELECT 1)",
        "UPDATE accounts SET balance=1 WHERE id IN (SELECT id FROM other)",
        "INSERT INTO accounts(id) VALUES(1); DELETE FROM accounts",
        "DELETE FROM accounts;;",
    ] {
        assert!(
            SqlRecognizerFactory::recognize(sql).is_err(),
            "accepted {sql}"
        );
    }
}

#[test]
fn conditional_comments_hints_and_mysql_mode_ambiguous_text_are_rejected() {
    for sql in [
        "DELETE /*!80000 FROM */ accounts",
        "DELETE FROM accounts /* explanation */",
        "DELETE FROM accounts -- comment",
        "DELETE FROM accounts # comment",
        "UPDATE /*+ hint */ accounts SET balance=1",
        "UPDATE accounts SET note='a\\b'",
        "UPDATE accounts SET note=\"double quoted\"",
        "UPDATE accounts SET note=1\0",
    ] {
        assert!(
            SqlRecognizerFactory::recognize(sql).is_err(),
            "accepted {sql:?}"
        );
    }
    let plan =
        SqlRecognizerFactory::recognize("UPDATE accounts SET note='/* ? */ -- # ;'").unwrap();
    assert_eq!(plan.parameter_count(), 0);
}

#[test]
fn duplicate_columns_and_ragged_values_are_rejected() {
    for sql in [
        "INSERT INTO accounts(id,ID) VALUES(1,2)",
        "INSERT INTO accounts(id,balance) VALUES(1),(2,3)",
        "UPDATE accounts SET balance=1,BALANCE=2",
        "INSERT INTO accounts(id) VALUES()",
        "INSERT INTO accounts(id) VALUES(balance)",
    ] {
        assert!(
            SqlRecognizerFactory::recognize(sql).is_err(),
            "accepted {sql}"
        );
    }
}

#[test]
fn exact_identifiers_are_validated_without_case_folding_or_sql_injection() {
    for sql in [
        "DELETE FROM `bad-name`",
        "DELETE FROM `schema.table`",
        "DELETE FROM `a``b`",
        "DELETE FROM `中文`",
        "DELETE FROM `1number`",
    ] {
        assert!(
            SqlRecognizerFactory::recognize(sql).is_err(),
            "accepted {sql}"
        );
    }
    assert_eq!(
        SqlRecognizerFactory::recognize("DELETE FROM `$Accounts_2`")
            .unwrap()
            .table_name(),
        "$Accounts_2"
    );
    let long = format!("DELETE FROM `{}`", "a".repeat(65));
    assert!(SqlRecognizerFactory::recognize(&long).is_err());
}

#[test]
fn raw_budget_is_enforced_before_parsing_and_errors_redact_sql_values() {
    let secret = "private-bound-value";
    let sql = format!("SELECT '{secret}'");
    let error = SqlRecognizerFactory::recognize(&sql).unwrap_err();
    assert!(!format!("{error:?} {error}").contains(secret));
    assert!(matches!(
        SqlRecognizerFactory::recognize(&" ".repeat(65_537)),
        Err(SqlError::Budget { .. })
    ));
    assert!(SqlRecognizerFactory::recognize("").is_err());
    assert!(SqlRecognizerFactory::recognize("  ;  ").is_err());
}

#[test]
fn deep_or_over_budget_expressions_are_rejected_without_stack_failure() {
    let deep = format!(
        "DELETE FROM accounts WHERE {}id=1{}",
        "(".repeat(80),
        ")".repeat(80)
    );
    assert!(SqlRecognizerFactory::recognize(&deep).is_err());
    let values = vec!["?"; 1025].join(",");
    let sql = format!("DELETE FROM accounts WHERE id IN ({values})");
    assert!(SqlRecognizerFactory::recognize(&sql).is_err());
    let accepted = format!(
        "DELETE FROM accounts WHERE id IN ({})",
        vec!["?"; 1024].join(",")
    );
    assert_eq!(
        SqlRecognizerFactory::recognize(&accepted)
            .unwrap()
            .parameter_count(),
        1024
    );
}

#[test]
fn public_debug_does_not_disclose_literal_sql_or_predicate_values() {
    let plan = SqlRecognizerFactory::recognize(
        "UPDATE accounts SET note='private-a' WHERE id='private-b'",
    )
    .unwrap();
    let debug = format!("{plan:?}");
    assert!(!debug.contains("private-a") && !debug.contains("private-b"));
    assert!(debug.contains("Update"));
}

#[test]
fn in_values_cannot_silently_expand_the_declared_constant_or_bound_value_matrix() {
    assert!(SqlRecognizerFactory::recognize("DELETE FROM accounts WHERE id IN (balance)").is_err());
}

#[test]
fn ignored_non_mysql_from_keyword_cannot_survive_as_an_accepted_original_sql() {
    for sql in [
        "UPDATE accounts SET balance=1 FROM",
        "UPDATE accounts SET balance=? FROM WHERE id=?",
    ] {
        assert!(
            SqlRecognizerFactory::recognize(sql).is_err(),
            "accepted {sql}"
        );
    }
}

#[test]
fn flat_operator_budget_is_checked_before_constructing_a_deep_ast() {
    let sql = format!(
        "DELETE FROM accounts WHERE {}",
        vec!["id=?"; 300].join(" OR ")
    );
    assert!(matches!(
        SqlRecognizerFactory::recognize(&sql),
        Err(SqlError::Budget {
            limit: "SQL operators"
        })
    ));
}

#[test]
fn unsupported_operator_chains_are_also_bounded_before_ast_construction() {
    for operator in ["||", "^", "&", "<<"] {
        let sql = format!(
            "DELETE FROM accounts WHERE {}",
            vec!["?"; 300].join(operator)
        );
        assert!(
            matches!(
                SqlRecognizerFactory::recognize(&sql),
                Err(SqlError::Budget {
                    limit: "SQL operators"
                })
            ),
            "unbounded {operator}"
        );
    }
}

#[test]
fn bare_mysql_builtins_are_not_misclassified_as_deterministic_columns() {
    for name in [
        "CURRENT_USER",
        "UTC_DATE",
        "UTC_TIME",
        "UTC_TIMESTAMP",
        "SYSTEM_USER",
        "SESSION_USER",
        "CURRENT_ROLE",
        "CURRENT_DATE",
        "LOCALTIME",
        "LOCALTIMESTAMP",
    ] {
        for sql in [
            format!("UPDATE accounts SET note={name} WHERE id=?"),
            format!("DELETE FROM accounts WHERE note={name}"),
        ] {
            assert!(
                SqlRecognizerFactory::recognize(&sql).is_err(),
                "accepted bare builtin {sql}"
            );
        }
        let sql = format!("UPDATE accounts SET note=`{name}` WHERE id=?");
        assert_eq!(
            SqlRecognizerFactory::recognize(&sql)
                .unwrap()
                .parameter_count(),
            1
        );
    }
}

#[test]
fn parser_consumed_modifiers_cannot_disappear_from_the_executable_contract() {
    for sql in [
        "INSERT LOCAL accounts(id) VALUES(?)",
        "INSERT INTO LOCAL accounts(id) VALUES(?)",
        "DELETE FROM accounts LIMIT ALL",
        "DELETE FROM accounts WHERE id=? LIMIT ALL",
        "INSERT INTO accounts(id) VALUES(?) LIMIT ALL",
    ] {
        assert!(
            SqlRecognizerFactory::recognize(sql).is_err(),
            "silently accepted {sql}"
        );
    }
}

#[test]
fn canonical_token_check_preserves_quotes_keyword_case_and_not_equal_spellings() {
    let sql = "update `Account` set note='O''Reilly; ?' where id != ?;";
    let plan = SqlRecognizerFactory::recognize(sql).unwrap();
    assert_eq!(plan.sql(), sql);
    assert_eq!(plan.table_name(), "Account");
    assert_eq!(plan.parameter_count(), 1);
}

#[test]
fn unsupported_div_syntax_and_non_dml_return_typed_errors_without_parser_panic() {
    for sql in [
        "UPDATE accounts SET balance=1 DIV",
        "DELETE FROM accounts WHERE id DIV",
        "SELECT 1 DIV",
        "INSERT INTO accounts(id) VALUES(1 DIV)",
        "UPDATE accounts SET balance=? DIV ?",
    ] {
        assert!(
            SqlRecognizerFactory::recognize(sql).is_err(),
            "accepted {sql}"
        );
    }
    assert_eq!(
        SqlRecognizerFactory::recognize("DELETE FROM accounts WHERE `DIV` = ?")
            .unwrap()
            .parameter_count(),
        1
    );
}
