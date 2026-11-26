use seata_datasource::{FieldValue, Row, TableRecords};
use std::collections::HashMap;
/// 完整物理镜像的字段/主键元组比较；不以字符串拼接键造成不同复合主键覆盖。
/// 对应 Java: org.apache.seata.rm.datasource.DataCompareUtils（既有完整镜像/原生值范围）。
pub(crate) struct DataCompareUtils;
impl DataCompareUtils {
    /// 参数为同一物理元数据的两份完整镜像；返回忽略行序、保持字段类型/值及DECIMAL scale的比较结果。
    /// 对应 Java: DataCompareUtils#isRecordsEquals/compareRows；不提供可空记录或未知JDBC类型的推断。
    pub(crate) fn records_equal(left: &TableRecords, right: &TableRecords) -> bool {
        if left.table_meta() != right.table_meta() || left.rows().len() != right.rows().len() {
            return false;
        }
        let map: HashMap<Vec<&FieldValue>, &Row> = left
            .rows()
            .iter()
            .map(|row| (row.primary_keys().map(|f| f.value()).collect(), row))
            .collect();
        map.len() == left.rows().len()
            && right.rows().iter().all(|row| {
                map.get(&row.primary_keys().map(|f| f.value()).collect::<Vec<_>>())
                    .is_some_and(|old| *old == row)
            })
    }
}
