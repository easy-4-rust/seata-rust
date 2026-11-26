//! INSERT模板必须保留真实参数位置，不能把字面量问号当参数。
#![allow(clippy::unwrap_used)]
use seata_at::SqlRecognizerFactory;

#[test]
fn templates_keep_original_row_column_and_binding_positions() {
    let parsed = SqlRecognizerFactory::recognize(
        "INSERT INTO t(a,b,c) VALUES (?, 'literal ?', (? + ?)), (7, ?, NULL)",
    )
    .unwrap();
    assert_eq!(parsed.parameter_count(), 4);
    let rows = parsed.insert_rows();
    assert_eq!(rows.len(), 2);
    let positions: Vec<_> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| (v.parameter_start(), v.parameter_count()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        positions,
        vec![vec![(0, 1), (1, 0), (1, 2)], vec![(3, 0), (3, 1), (4, 0)]]
    );
    assert_eq!(rows[0][0].sql(), "?");
    assert_eq!(rows[0][1].sql(), "'literal ?'");
    assert_eq!(rows[1][2].sql(), "NULL");
}
#[test]
fn non_insert_recognizers_have_no_value_templates() {
    for sql in ["UPDATE t SET a=? WHERE b=?", "DELETE FROM t WHERE b=?"] {
        assert!(SqlRecognizerFactory::recognize(sql)
            .unwrap()
            .insert_rows()
            .is_empty());
    }
}
#[test]
fn template_debug_does_not_disclose_literals() {
    let parsed =
        SqlRecognizerFactory::recognize("INSERT INTO t(a) VALUES ('private-cell-123')").unwrap();
    assert!(!format!("{:?}", parsed.insert_rows()).contains("private-cell-123"));
}
