use std::fmt;

/// SQL 方法调用表达式，记录方法内部占位符数量，零计数提供共享实例。
/// 对应 Java: org.apache.seata.sqlparser.struct.SqlMethodExpr
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SqlMethodExpr {
    placeholder_count: i32,
}

static INSTANCE: SqlMethodExpr = SqlMethodExpr::new(0);

impl SqlMethodExpr {
    /// 返回零占位符共享实例的引用；无参数。对应 Java: SqlMethodExpr#get
    pub fn get() -> &'static Self {
        &INSTANCE
    }

    /// 保存有符号 placeholder_count，返回表达式；与原构造器相同，不拒绝负数。
    pub const fn new(placeholder_count: i32) -> Self {
        Self { placeholder_count }
    }

    /// 返回方法内部原始占位符计数；无参数。对应 Java: SqlMethodExpr#getPlaceholderCount
    pub const fn placeholder_count(&self) -> i32 {
        self.placeholder_count
    }

    /// 返回 Java Integer.hashCode 结果；无参数。Rust Hash 不承诺相同整数输出。
    pub const fn java_hash_code(&self) -> i32 {
        self.placeholder_count
    }
}

impl fmt::Display for SqlMethodExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SQL_METHOD")
    }
}
