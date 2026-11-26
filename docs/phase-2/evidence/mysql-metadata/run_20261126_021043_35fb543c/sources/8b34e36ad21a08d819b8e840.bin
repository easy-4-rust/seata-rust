use std::fmt;

/// SQL DEFAULT 表达式的唯一共享标记，不与空值或非占位符标记合并。
/// 对应 Java: org.apache.seata.sqlparser.struct.SqlDefaultExpr
#[derive(Debug)]
pub struct SqlDefaultExpr {
    _private: (),
}

static INSTANCE: SqlDefaultExpr = SqlDefaultExpr { _private: () };

impl SqlDefaultExpr {
    /// 返回 DEFAULT 标记的共享引用；无参数。对应 Java: SqlDefaultExpr#get
    pub fn get() -> &'static Self {
        &INSTANCE
    }
}

impl fmt::Display for SqlDefaultExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DEFAULT")
    }
}
