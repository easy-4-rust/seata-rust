//! ORM 读取必须先通过独立只读语法与分页预算检查。
#![allow(clippy::unwrap_used)]
use seata_at::MySqlSelectRecognizer;
use seata_datasource::FieldValue;
#[test]
fn reads_keep_projection_order_bindings_and_explicit_limits() {
    let sql=MySqlSelectRecognizer::recognize("SELECT `items`.`v` AS `value`, `items`.`id` FROM `items` WHERE `items`.`id` = ? ORDER BY `items`.`id` DESC LIMIT ? OFFSET ?").unwrap();
    assert_eq!(sql.table_name(), "items");
    assert_eq!(sql.parameter_count(), 3);
    assert_eq!(
        sql.columns(),
        &[("v".into(), "value".into()), ("id".into(), "id".into())]
    );
    sql.validate_binding_bounds(&[
        FieldValue::Signed(1),
        FieldValue::Unsigned(10),
        FieldValue::Unsigned(0),
    ])
    .unwrap();
    assert!(sql
        .validate_binding_bounds(&[
            FieldValue::Signed(1),
            FieldValue::Unsigned(1025),
            FieldValue::Unsigned(0)
        ])
        .is_err());
    assert!(sql
        .validate_binding_bounds(&[
            FieldValue::Signed(1),
            FieldValue::Signed(-1),
            FieldValue::Unsigned(0)
        ])
        .is_err());
}
#[test]
fn reads_reject_mutation_functions_joins_and_parser_discarded_tokens() {
    for sql in [
        "SELECT SLEEP(1) FROM items",
        "SELECT GET_LOCK('key',1) FROM items",
        "SELECT @a:=id FROM items",
        "SELECT id INTO OUTFILE '/tmp/leak' FROM items",
        "SELECT id FROM items FOR UPDATE",
        "SELECT id FROM items; DELETE FROM items",
        "SELECT /*!50000 SLEEP(1), */ id FROM items",
        "SELECT id FROM items JOIN other ON items.id=other.id",
        "SELECT id FROM (SELECT id FROM items) t",
        "SELECT CURRENT_USER FROM items",
        "SELECT id FROM items LIMIT 1025",
        "SELECT id FROM items LIMIT 1 OFFSET 1000001",
        "SELECT id,id FROM items",
        "SELECT wrong.id FROM items",
    ] {
        assert!(MySqlSelectRecognizer::recognize(sql).is_err(), "{sql}");
    }
    let sql =
        MySqlSelectRecognizer::recognize("SELECT id FROM items WHERE id = 'private_read_literal'")
            .unwrap();
    assert!(!format!("{sql:?}").contains("private_read_literal"));
}
