/// SQL识别错误；仅保存静态原因，不携带业务SQL、字面值或驱动凭据。
/// 对应 Java: org.apache.seata.sqlparser.SQLParsingException；增加明确预算分类。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SqlError {
    /// SQL不符合已声明的语法形状。
    #[error("SQL does not match the selected MySQL grammar")]
    Malformed,
    /// 参数或标识符不满足AT契约。
    #[error("invalid SQL contract: {reason}")]
    Invalid {
        /// 固定原因，不包含用户输入。
        reason: &'static str,
    },
    /// AST有效但AT不能安全执行该功能。
    #[error("unsupported AT SQL: {feature}")]
    Unsupported {
        /// 固定功能名称。
        feature: &'static str,
    },
    /// 在分配或递归前触发的有界资源预算。
    #[error("SQL budget exceeded: {limit}")]
    Budget {
        /// 固定预算名称。
        limit: &'static str,
    },
}
