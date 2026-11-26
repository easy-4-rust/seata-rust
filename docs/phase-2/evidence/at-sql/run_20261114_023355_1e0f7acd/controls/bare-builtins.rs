use crate::SqlError;
use sqlparser::ast::{
    BinaryOperator, Expr, Ident, ObjectName, TableFactor, TableWithJoins, UnaryOperator, Value,
};
use std::collections::HashSet;

/// MySQL表/列与表达式的共同AT安全检查，不把解析器全部语法视为已支持。
/// 对应 Java: org.apache.seata.sqlparser.druid.mysql.BaseMySQLRecognizer。
pub(crate) struct BaseMySqlRecognizer;
impl BaseMySqlRecognizer {
    pub(crate) fn identifier(value: &Ident) -> Result<String, SqlError> {
        let bytes = value.value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 64
            || !matches!(value.quote_style, None | Some('`'))
            || !bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$')
            || bytes[0].is_ascii_digit()
        {
            return Err(SqlError::Invalid {
                reason: "bounded ASCII MySQL identifier required",
            });
        }
        Ok(value.value.clone())
    }
    pub(crate) fn object(value: &ObjectName) -> Result<String, SqlError> {
        if value.0.len() != 1 {
            return Err(SqlError::Unsupported {
                feature: "qualified/cross-schema identifiers",
            });
        }
        Self::identifier(&value.0[0])
    }
    pub(crate) fn columns(values: &[Ident]) -> Result<Vec<String>, SqlError> {
        if values.is_empty() || values.len() > 1024 {
            return Err(SqlError::Budget {
                limit: "1..1024 columns",
            });
        }
        let mut seen = HashSet::new();
        values
            .iter()
            .map(|value| {
                let name = Self::identifier(value)?;
                if !seen.insert(name.to_ascii_lowercase()) {
                    return Err(SqlError::Invalid {
                        reason: "duplicate column",
                    });
                }
                Ok(name)
            })
            .collect()
    }
    pub(crate) fn table(table: &TableWithJoins) -> Result<String, SqlError> {
        if !table.joins.is_empty() {
            return Err(SqlError::Unsupported {
                feature: "joined DML",
            });
        }
        match &table.relation {
            TableFactor::Table {
                name,
                alias: None,
                args: None,
                with_hints,
                version: None,
                with_ordinality: false,
                partitions,
            } if with_hints.is_empty() && partitions.is_empty() => Self::object(name),
            _ => Err(SqlError::Unsupported {
                feature: "table alias/function/hint/partition",
            }),
        }
    }
    pub(crate) fn expression(
        value: &Expr,
        parameters: &mut usize,
        depth: usize,
        columns: bool,
    ) -> Result<(), SqlError> {
        if depth > 32 {
            return Err(SqlError::Budget {
                limit: "expression depth",
            });
        }
        let mut child = |value: &Expr| Self::expression(value, parameters, depth + 1, columns);
        match value {
            Expr::Identifier(name) if columns => {
                // 上游MySQL AST会将部分无括号内建项表示成Identifier，不能据此当作稳定列。
                if false
                    && matches!(
                        name.value.to_ascii_uppercase().as_str(),
                        "CURRENT_USER"
                            | "SESSION_USER"
                            | "SYSTEM_USER"
                            | "CURRENT_ROLE"
                            | "CURRENT_DATE"
                            | "CURRENT_TIME"
                            | "CURRENT_TIMESTAMP"
                            | "LOCALTIME"
                            | "LOCALTIMESTAMP"
                            | "UTC_DATE"
                            | "UTC_TIME"
                            | "UTC_TIMESTAMP"
                    )
                {
                    return Err(SqlError::Unsupported {
                        feature: "unquoted MySQL builtin identifier",
                    });
                }
                Self::identifier(name)?;
            }
            Expr::Value(Value::Placeholder(marker)) if marker == "?" => {
                *parameters += 1;
                if *parameters > 1024 {
                    return Err(SqlError::Budget {
                        limit: "parameters",
                    });
                }
            }
            Expr::Value(
                Value::Number(_, false)
                | Value::SingleQuotedString(_)
                | Value::Null
                | Value::Boolean(_),
            ) => {}
            Expr::Nested(value) => child(value)?,
            Expr::UnaryOp {
                op: UnaryOperator::Plus | UnaryOperator::Minus | UnaryOperator::Not,
                expr,
            } => child(expr)?,
            Expr::BinaryOp {
                left,
                op:
                    BinaryOperator::Plus
                    | BinaryOperator::Minus
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::Eq
                    | BinaryOperator::NotEq
                    | BinaryOperator::Lt
                    | BinaryOperator::LtEq
                    | BinaryOperator::Gt
                    | BinaryOperator::GtEq
                    | BinaryOperator::And
                    | BinaryOperator::Or,
                right,
            } => {
                child(left)?;
                child(right)?;
            }
            Expr::IsNull(value) | Expr::IsNotNull(value) => child(value)?,
            Expr::Between {
                expr, low, high, ..
            } => {
                child(expr)?;
                child(low)?;
                child(high)?;
            }
            Expr::InList { expr, list, .. } if !list.is_empty() && list.len() <= 1024 => {
                child(expr)?;
                for item in list {
                    Self::expression(item, parameters, depth + 1, false)?;
                }
            }
            _ => {
                return Err(SqlError::Unsupported {
                    feature: "expression/function/subquery/variable",
                })
            }
        }
        Ok(())
    }
    pub(crate) fn predicate(
        value: &Option<Expr>,
        parameters: &mut usize,
    ) -> Result<Option<String>, SqlError> {
        if let Some(value) = value {
            Self::expression(value, parameters, 0, true)?;
            Ok(Some(value.to_string()))
        } else {
            Ok(None)
        }
    }
}
