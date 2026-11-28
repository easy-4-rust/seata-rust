/// 数据库标识符的左右转义码元；Java char 使用 u16 保留全部 UTF-16 单码元。
/// 对应 Java: org.apache.seata.sqlparser.EscapeSymbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EscapeSymbol {
    left_symbol: u16,
    right_symbol: u16,
}

impl EscapeSymbol {
    /// 用同一个 single_symbol 作为左右码元，返回转义符对；包括非 Unicode 标量码元。
    pub const fn new(single_symbol: u16) -> Self {
        Self::paired(single_symbol, single_symbol)
    }

    /// 接收 left_symbol、right_symbol 原始码元并返回转义符对，不替换或归一化。
    pub const fn paired(left_symbol: u16, right_symbol: u16) -> Self {
        Self {
            left_symbol,
            right_symbol,
        }
    }

    /// 返回左侧 UTF-16 码元；无参数。对应 Java: EscapeSymbol#getLeftSymbol
    pub const fn left_symbol(&self) -> u16 {
        self.left_symbol
    }

    /// 返回右侧 UTF-16 码元；无参数。对应 Java: EscapeSymbol#getRightSymbol
    pub const fn right_symbol(&self) -> u16 {
        self.right_symbol
    }
}
