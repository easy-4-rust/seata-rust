//! 对应原 Java: org.apache.seata.rm.datasource.undo.mysql.MySQLUndoInsertExecutorTest。
use super::{
    mysql_undo_executor_source::{original_log, record},
    MySqlUndoInsertExecutor,
};
use std::error::Error;

#[test]
fn original_mysql_undo_insert_build_undo_sql() -> Result<(), Box<dyn Error>> {
    let log = original_log()?;
    let rows = MySqlUndoInsertExecutor::get_undo_rows(&log);
    let sql = MySqlUndoInsertExecutor::build_undo_sql(rows.table_meta())?.to_lowercase();
    let mut values = vec!["not_null:true".to_owned()];
    let has_verb = sql.contains("delete");
    assert!(has_verb);
    values.push(format!("true:{has_verb}"));
    let has_id = sql.contains("id");
    assert!(has_id);
    values.push(format!("true:{has_id}"));
    record("MySQLUndoInsertExecutorTest", "buildUndoSQL", &values)
}

#[test]
fn original_mysql_undo_insert_get_undo_rows() -> Result<(), Box<dyn Error>> {
    let log = original_log()?;
    let selected = MySqlUndoInsertExecutor::get_undo_rows(&log);
    let same = std::ptr::eq(selected, log.after_image());
    assert!(same);
    record(
        "MySQLUndoInsertExecutorTest",
        "getUndoRows",
        &[format!("equals:object:{same}")],
    )
}
