use crate::JavaString;

/// SQL 序列表达式，序列名和函数名初始均为空引用，允许分别修改。
/// 对应 Java: org.apache.seata.sqlparser.struct.SqlSequenceExpr
#[derive(Debug, Default)]
pub struct SqlSequenceExpr {
    sequence: Option<JavaString>,
    function: Option<JavaString>,
}

impl SqlSequenceExpr {
    /// 接收可空 sequence 和 function 并原样保存，返回表达式；无参构造使用 Default。
    /// 对应 Java: SqlSequenceExpr(String, String)
    pub fn new(sequence: Option<String>, function: Option<String>) -> Self {
        Self::from_java(sequence.map(Into::into), function.map(Into::into))
    }

    /// 返回序列名的 UTF-8 便利视图；无参数。null或孤立代理码元返回None；无损读取用 sequence_java。
    /// 对应 Java: SqlSequenceExpr#getSequence 的Rust便利视图，原始语义由sequence_java提供。
    pub fn sequence(&self) -> Option<&str> {
        self.sequence.as_ref().and_then(JavaString::as_str)
    }

    /// 用可空 sequence 替换序列名，无返回值。对应 Java: SqlSequenceExpr#setSequence
    pub fn set_sequence(&mut self, sequence: Option<String>) {
        self.set_sequence_java(sequence.map(Into::into));
    }

    /// 返回函数名的 UTF-8 便利视图；无参数。null或孤立代理码元返回None；无损读取用 function_java。
    /// 对应 Java: SqlSequenceExpr#getFunction 的Rust便利视图，原始语义由function_java提供。
    pub fn function(&self) -> Option<&str> {
        self.function.as_ref().and_then(JavaString::as_str)
    }

    /// 用可空 function 替换函数名，无返回值。对应 Java: SqlSequenceExpr#setFunction
    pub fn set_function(&mut self, function: Option<String>) {
        self.set_function_java(function.map(Into::into));
    }

    /// 接收原始可空 sequence、function 并返回表达式。对应 Java: SqlSequenceExpr(String,String)
    pub fn from_java(sequence: Option<JavaString>, function: Option<JavaString>) -> Self {
        Self { sequence, function }
    }

    /// 返回原始可空序列名，无参数且无损。对应 Java: SqlSequenceExpr#getSequence
    pub fn sequence_java(&self) -> Option<&JavaString> {
        self.sequence.as_ref()
    }

    /// 用可空 sequence 无损替换序列名，无返回值。对应 Java: SqlSequenceExpr#setSequence
    pub fn set_sequence_java(&mut self, sequence: Option<JavaString>) {
        self.sequence = sequence;
    }

    /// 返回原始可空函数名，无参数且无损。对应 Java: SqlSequenceExpr#getFunction
    pub fn function_java(&self) -> Option<&JavaString> {
        self.function.as_ref()
    }

    /// 用可空 function 无损替换函数名，无返回值。对应 Java: SqlSequenceExpr#setFunction
    pub fn set_function_java(&mut self, function: Option<JavaString>) {
        self.function = function;
    }
}
