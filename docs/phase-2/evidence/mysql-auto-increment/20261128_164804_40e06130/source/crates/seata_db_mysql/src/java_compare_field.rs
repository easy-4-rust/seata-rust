use crate::JavaCompareValue;
use seata_sqlparser::JavaString;

/// 原对象比较使用的字段，保留原 JDBC 类型和可空列名。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field 的比较输入。
#[derive(Clone)]
pub struct JavaCompareField {
    pub(crate) name: Option<JavaString>,
    pub(crate) jdbc_type: i32,
    pub(crate) value: JavaCompareValue,
}

impl JavaCompareField {
    /// 返回字段当前的原值，包括 Fastjson 比较已原地转换的结果。
    pub fn value(&self) -> &JavaCompareValue {
        &self.value
    }
    /// 参数为原列名、JDBC 类型编号及值，返回保留原类型的字段。
    pub fn new(name: Option<JavaString>, jdbc_type: i32, value: JavaCompareValue) -> Self {
        Self {
            name,
            jdbc_type,
            value,
        }
    }
}
