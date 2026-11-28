//! COUNT 只允许密封的内建统计和单层纯读取，不能复用数据读取的行截断。
#![allow(clippy::unwrap_used)]
use seata_at::{MySqlCountRecognizer, MySqlSelectRecognizer, SqlRecognizerFactory};
use seata_datasource::FieldValue;
#[test]
fn native_count_and_derived_count_preserve_bindings_without_a_probe_limit() {
    let count=MySqlCountRecognizer::recognize("SELECT COUNT(*) AS num_items FROM (SELECT id,name FROM items WHERE name LIKE ?) AS sub_query").unwrap();
    assert_eq!(count.table_name(), "items");
    assert_eq!(count.parameter_count(), 1);
    assert!(!count.sql().contains("1025"));
    count
        .validate_binding_bounds(&[FieldValue::Text("alpha%".into())])
        .unwrap();
    let page=MySqlCountRecognizer::recognize("SELECT COUNT(*) FROM (SELECT id FROM items WHERE name NOT LIKE ? LIMIT ? OFFSET ?) AS sub_query").unwrap();
    assert_eq!(page.parameter_count(), 3);
    assert!(page.sql().contains("LIMIT ? OFFSET ?"));
    assert!(page
        .validate_binding_bounds(&[
            FieldValue::Text("x%".into()),
            FieldValue::Unsigned(1025),
            FieldValue::Unsigned(0)
        ])
        .is_err());
}
#[test]
fn unsafe_count_shapes_and_like_side_effects_are_rejected() {
    for sql in [
        "SELECT schema_name.COUNT(*) FROM items",
        "SELECT `COUNT`(*) FROM items",
        "SELECT COUNT(1) FROM items",
        "SELECT COUNT(DISTINCT id) FROM items",
        "SELECT COUNT(*) OVER() FROM items",
        "SELECT COUNT(*) FROM (SELECT SLEEP(1) FROM items) AS x",
        "SELECT COUNT(*) FROM (SELECT id FROM items) AS x WHERE x.id>1",
        "SELECT COUNT(*) FROM (SELECT id FROM (SELECT id FROM items) AS y) AS x",
        "SELECT COUNT(*) FROM items GROUP BY id",
        "SELECT COUNT(*) FROM items LIMIT 1",
        "SELECT COUNT(*),id FROM items",
    ] {
        assert!(MySqlCountRecognizer::recognize(sql).is_err(), "{sql}");
    }
    for sql in [
        "SELECT id FROM items WHERE name LIKE LAST_INSERT_ID(1)",
        "SELECT id FROM items WHERE name LIKE ? ESCAPE 'xx'",
        "SELECT id FROM items WHERE name ILIKE ?",
    ] {
        assert!(MySqlSelectRecognizer::recognize(sql).is_err(), "{sql}");
    }
    assert!(
        MySqlSelectRecognizer::recognize("SELECT id FROM items WHERE name LIKE ? ESCAPE '!'")
            .is_ok()
    );
    assert!(SqlRecognizerFactory::recognize("DELETE FROM items WHERE name LIKE ?").is_err());
}
