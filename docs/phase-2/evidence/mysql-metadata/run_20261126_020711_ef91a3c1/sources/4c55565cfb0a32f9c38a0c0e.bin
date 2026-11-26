use crate::InvalidEnumValue;
use std::fmt;

/// 完整的IndexType分类及原始编号，不表示相应SQL已获准执行。
/// 对应 Java: org.apache.seata.sqlparser.struct.IndexType
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum IndexType {
    /// 主键索引，原编号0。
    PRIMARY = 0,
    /// 普通索引，原编号1。
    NORMAL = 1,
    /// 唯一索引，原编号2。
    UNIQUE = 2,
    /// 全文索引，原编号3。
    FULL_TEXT = 3,
}

impl IndexType {
    /// 按原 Java 声明顺序排列的完整分类，等同 values()。
    pub const ALL: [Self; 4] = [Self::PRIMARY, Self::NORMAL, Self::UNIQUE, Self::FULL_TEXT];

    /// 返回固定 Java 分类编号；无参数。对应 Java: IndexType#value
    pub const fn value(self) -> i32 {
        self as i32
    }
}

impl TryFrom<i32> for IndexType {
    type Error = InvalidEnumValue;

    /// 用原始整数 i 查询分类；非法值返回保留原消息的错误，不猜测默认分类。
    fn try_from(i: i32) -> Result<Self, Self::Error> {
        match i {
            0 => Ok(Self::PRIMARY),
            1 => Ok(Self::NORMAL),
            2 => Ok(Self::UNIQUE),
            3 => Ok(Self::FULL_TEXT),
            _ => Err(InvalidEnumValue::new("IndexType", i)),
        }
    }
}

impl fmt::Display for IndexType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 原枚举 toString 返回声明名，不能改成数据库SQL拼写。
        fmt::Debug::fmt(self, f)
    }
}
