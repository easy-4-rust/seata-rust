//! 对应原 Java: org.apache.seata.rm.datasource.undo.mysql.MySQLUndoUpdateExecutorTest。
use super::{
    mysql_undo_executor_source::{original_log, record},
    MySqlUndoUpdateExecutor,
};
use std::error::Error;

#[test]
fn original_mysql_undo_update_build_undo_sql() -> Result<(), Box<dyn Error>> {
    let log = original_log()?;
    let rows = MySqlUndoUpdateExecutor::get_undo_rows(&log);
    let row = rows.rows().first().ok_or("original first row")?;
    let sql = MySqlUndoUpdateExecutor::build_undo_sql(rows.table_meta(), row)?.to_lowercase();
    let mut values = vec!["not_null:true".to_owned()];
    let has_verb = sql.contains("update");
    assert!(has_verb);
    values.push(format!("true:{has_verb}"));
    let has_id = sql.contains("id");
    assert!(has_id);
    values.push(format!("true:{has_id}"));
    let has_age = sql.contains("age");
    assert!(has_age);
    values.push(format!("true:{has_age}"));
    record("MySQLUndoUpdateExecutorTest", "buildUndoSQL", &values)
}

#[test]
fn original_mysql_undo_update_get_undo_rows() -> Result<(), Box<dyn Error>> {
    let log = original_log()?;
    let selected = MySqlUndoUpdateExecutor::get_undo_rows(&log);
    let same = std::ptr::eq(selected, log.before_image());
    assert!(same);
    record(
        "MySQLUndoUpdateExecutorTest",
        "getUndoRows",
        &[format!("equals:object:{same}")],
    )
}
