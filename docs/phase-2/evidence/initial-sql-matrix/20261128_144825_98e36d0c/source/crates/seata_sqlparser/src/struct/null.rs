use std::fmt;

/// SQL NULL 的共享标记，与 Rust 缺失引用 None 保持不同的类型意义。
/// 对应 Java: org.apache.seata.sqlparser.struct.Null
#[derive(Debug)]
pub struct Null {
    _private: (),
}

static INSTANCE: Null = Null { _private: () };

impl Null {
    /// 返回 SQL NULL 标记的共享引用；无参数。对应 Java: Null#get
    pub fn get() -> &'static Self {
        &INSTANCE
    }
}

impl fmt::Display for Null {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NULL")
    }
}
