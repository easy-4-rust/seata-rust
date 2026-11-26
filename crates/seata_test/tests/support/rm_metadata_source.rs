//! 原测试的共享观察与夹具；Rust 辅助，不计为额外 Java 迁移对象。
use seata_sqlparser::{ColumnList, ColumnMeta, IndexMeta, IndexType, MetadataLocale, TableMeta};
use std::{error::Error, fmt::Write as FmtWrite, io::Write, sync::Mutex};
/// 测试返回的实际错误，不把失败隐藏为记录缺失。
pub type TestResult = Result<(), Box<dyn Error>>;
static OUTPUT: Mutex<()> = Mutex::new(());
/// 参数为原类、方法与实际断言序列；可选保存观察结果，返回写入结果。
pub fn observe(class: &str, method: &str, values: &[String]) -> TestResult {
    if let Some(path) = std::env::var_os("SEATA_RM_METADATA_OBSERVATIONS") {
        let _guard = OUTPUT.lock().map_err(|_| "observation lock poisoned")?;
        let mut bytes = String::new();
        for byte in values.join("\u{1f}").bytes() {
            write!(&mut bytes, "{byte:02x}")?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(
            file,
            "org.apache.seata.rm.datasource.sql.struct.{class}#{method}\t{bytes}"
        )?;
    }
    Ok(())
}
/// 参数为记录列表、断言类型、实际值及期望值；断言后保存实际值，无返回值。
pub fn check(values: &mut Vec<String>, kind: &str, actual: bool, expected: bool) {
    assert_eq!(actual, expected, "original {kind} assertion");
    values.push(format!("{kind}:{actual}"));
}
/// 无参数；建立与原测试 en 默认语言相同的表元数据，返回新对象。
pub fn table() -> TableMeta {
    TableMeta::new(MetadataLocale::new("en"))
}
/// 参数为原可空列名；返回只有该列的主键索引或元数据错误。
pub fn primary(column: Option<&str>) -> Result<IndexMeta, Box<dyn Error>> {
    let c = ColumnMeta::default();
    c.set_column_name(column.map(Into::into))?;
    let index = IndexMeta::default();
    index.set_indextype(Some(IndexType::PRIMARY))?;
    index.set_values(Some(ColumnList::from_values(vec![Some(c)])))?;
    Ok(index)
}
