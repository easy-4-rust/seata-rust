//! 对应原 Java: org.apache.seata.rm.datasource.undo.mysql.MySQLUndoDeleteExecutorTest。
use super::{
    mysql_undo_executor_source::{original_log, record},
    MySqlUndoDeleteExecutor,
};
use std::error::Error;

#[test]
fn original_mysql_undo_delete_build_undo_sql() -> Result<(), Box<dyn Error>> {
    let log = original_log()?;
    let rows = MySqlUndoDeleteExecutor::get_undo_rows(&log);
    let row = rows.rows().first().ok_or("original first row")?;
    let sql = MySqlUndoDeleteExecutor::build_undo_sql(rows.table_meta(), row)?.to_lowercase();
    let mut values = vec!["not_null:true".to_owned()];
    let has_verb = sql.contains("insert");
    assert!(has_verb);
    values.push(format!("true:{has_verb}"));
    let has_id = sql.contains("id");
    assert!(has_id);
    values.push(format!("true:{has_id}"));
    record("MySQLUndoDeleteExecutorTest", "buildUndoSQL", &values)
}

#[test]
fn original_mysql_undo_delete_get_undo_rows() -> Result<(), Box<dyn Error>> {
    let log = original_log()?;
    let selected = MySqlUndoDeleteExecutor::get_undo_rows(&log);
    let same = std::ptr::eq(selected, log.before_image());
    assert!(same);
    record(
        "MySQLUndoDeleteExecutorTest",
        "getUndoRows",
        &[format!("equals:object:{same}")],
    )
}
