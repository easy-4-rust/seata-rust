use std::fmt;

/// 表示表达式并非参数占位符的共享标记，不代替 SQL NULL 或 DEFAULT。
/// 对应 Java: org.apache.seata.sqlparser.struct.NotPlaceholderExpr
#[derive(Debug)]
pub struct NotPlaceholderExpr {
    _private: (),
}

static INSTANCE: NotPlaceholderExpr = NotPlaceholderExpr { _private: () };

impl NotPlaceholderExpr {
    /// 返回非占位符标记的共享引用；无参数。对应 Java: NotPlaceholderExpr#get
    pub fn get() -> &'static Self {
        &INSTANCE
    }
}

impl fmt::Display for NotPlaceholderExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NOT_PLACEHOLDER")
    }
}
