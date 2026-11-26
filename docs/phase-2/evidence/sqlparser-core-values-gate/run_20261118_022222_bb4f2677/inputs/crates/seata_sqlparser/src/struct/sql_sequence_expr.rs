/// SQL 序列表达式，序列名和函数名初始均为空引用，允许分别修改。
/// 对应 Java: org.apache.seata.sqlparser.struct.SqlSequenceExpr
#[derive(Debug, Default)]
pub struct SqlSequenceExpr {
    sequence: Option<String>,
    function: Option<String>,
}

impl SqlSequenceExpr {
    /// 接收可空 sequence 和 function 并原样保存，返回表达式；无参构造使用 Default。
    /// 对应 Java: SqlSequenceExpr(String, String)
    pub fn new(sequence: Option<String>, function: Option<String>) -> Self {
        Self { sequence, function }
    }

    /// 返回可空序列名的借用；无参数。对应 Java: SqlSequenceExpr#getSequence
    pub fn sequence(&self) -> Option<&str> {
        self.sequence.as_deref()
    }

    /// 用可空 sequence 替换序列名，无返回值。对应 Java: SqlSequenceExpr#setSequence
    pub fn set_sequence(&mut self, sequence: Option<String>) {
        self.sequence = sequence;
    }

    /// 返回可空函数名的借用；无参数。对应 Java: SqlSequenceExpr#getFunction
    pub fn function(&self) -> Option<&str> {
        self.function.as_deref()
    }

    /// 用可空 function 替换函数名，无返回值。对应 Java: SqlSequenceExpr#setFunction
    pub fn set_function(&mut self, function: Option<String>) {
        self.function = function;
    }
}
