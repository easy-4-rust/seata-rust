use super::JacksonUndoFieldValue;

/// 独立 Jackson JDBC 字段的原对象投影，保留 JDBC 类型、主键标记和装箱值。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field。
#[derive(Clone, PartialEq)]
pub struct JacksonUndoField {
    pub(super) name: String,
    pub(super) jdbc_type: i32,
    pub(super) primary: bool,
    pub(super) value: JacksonUndoFieldValue,
}

impl JacksonUndoField {
    /// 参数为列名、JDBC 编号、主键标记及带类语义的值，返回字段对象。
    pub fn new(name: String, jdbc_type: i32, primary: bool, value: JacksonUndoFieldValue) -> Self {
        Self {
            name,
            jdbc_type,
            primary,
            value,
        }
    }

    /// 无参数；返回记录中的原 JDBC 列名。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 无参数；返回原 JDBC 编号。
    pub fn jdbc_type(&self) -> i32 {
        self.jdbc_type
    }

    /// 无参数；返回原字段是否标记为主键。
    pub fn is_primary_key(&self) -> bool {
        self.primary
    }

    /// 无参数；返回装箱值的引用。
    pub fn value(&self) -> &JacksonUndoFieldValue {
        &self.value
    }
}
