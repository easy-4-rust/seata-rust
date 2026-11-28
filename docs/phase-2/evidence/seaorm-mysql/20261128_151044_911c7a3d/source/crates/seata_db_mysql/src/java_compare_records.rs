use crate::JavaCompareField;
use seata_sqlparser::JavaString;

/// 原对象比较用的记录视图，保留原表名、主键列清单及可空行集合。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.TableRecords 的比较输入。
pub struct JavaCompareRecords<'a> {
    pub(crate) table_name: JavaString,
    pub(crate) primary_keys: &'a [JavaString],
    pub(crate) rows: Option<&'a [Vec<JavaCompareField>]>,
}

impl<'a> JavaCompareRecords<'a> {
    /// 参数为表名、主键列清单和可空行集合，返回借用式记录。
    pub fn new(
        table_name: JavaString,
        primary_keys: &'a [JavaString],
        rows: Option<&'a [Vec<JavaCompareField>]>,
    ) -> Self {
        Self {
            table_name,
            primary_keys,
            rows,
        }
    }
}

/// 保留可空表名、可空主键元数据与可写行的记录视图。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.TableRecords 的原地比较输入。
pub struct JavaCompareRecordsMut<'a> {
    pub(crate) table_name: Option<JavaString>,
    pub(crate) primary_keys: Option<&'a [JavaString]>,
    pub(crate) rows: Option<&'a mut [Vec<JavaCompareField>]>,
}

impl<'a> JavaCompareRecordsMut<'a> {
    /// 参数为原表名、元数据中有序主键列清单和原行，返回允许原地转换的视图。
    /// 主键元数据缺失与存在但没有主键是不同输入。
    pub fn new(
        table_name: Option<JavaString>,
        primary_keys: Option<&'a [JavaString]>,
        rows: Option<&'a mut [Vec<JavaCompareField>]>,
    ) -> Self {
        Self {
            table_name,
            primary_keys,
            rows,
        }
    }
}
