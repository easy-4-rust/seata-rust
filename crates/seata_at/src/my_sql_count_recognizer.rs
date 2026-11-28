use crate::{
    mysql::BaseMySqlRecognizer as Base, MySqlSelectRecognizer, SqlError, SqlRecognizerFactory,
};
use seata_datasource::FieldValue;
use sqlparser::{
    ast::{
        Expr, FunctionArg, FunctionArgExpr, FunctionArguments, SelectItem, SetExpr, Statement,
        TableFactor,
    },
    dialect::MySqlDialect,
    parser::Parser,
};
use std::fmt;
/// 只允许内建 COUNT(*) 与一层纯单表派生查询的密封统计计划，不截断统计输入。
/// 对应 Java: 无直接对象；Rust ORM Paginator 的有界统计扩展。
pub struct MySqlCountRecognizer {
    sql: String,
    inner: MySqlSelectRecognizer,
}
impl MySqlCountRecognizer {
    /// 参数为原统计 SQL；返回精确单项 COUNT(*) 计划，不接收 UDF、窗口或通用子查询。
    pub fn recognize(sql: &str) -> Result<Self, SqlError> {
        let dialect = MySqlDialect {};
        let tokens = SqlRecognizerFactory::checked_select_tokens(sql, &dialect)?;
        let statements = Parser::new(&dialect)
            .with_recursion_limit(64)
            .with_tokens(tokens.clone())
            .parse_statements()
            .map_err(|_| SqlError::Malformed)?;
        let [statement] = statements.as_slice() else {
            return Err(SqlError::Invalid {
                reason: "one count statement required",
            });
        };
        let Statement::Query(query) = statement else {
            return Err(SqlError::Unsupported {
                feature: "non query count",
            });
        };
        if query.order_by.is_some() || query.limit.is_some() || query.offset.is_some() {
            return Err(SqlError::Unsupported {
                feature: "outer count pagination/order",
            });
        }
        let SetExpr::Select(select) = query.body.as_ref() else {
            return Err(SqlError::Unsupported {
                feature: "count set operation",
            });
        };
        if select.projection.len() != 1
            || select.from.len() != 1
            || !select.from[0].joins.is_empty()
        {
            return Err(SqlError::Unsupported {
                feature: "single count projection/table required",
            });
        }
        let expr = match &select.projection[0] {
            SelectItem::UnnamedExpr(expr) => expr,
            SelectItem::ExprWithAlias { expr, alias } => {
                Base::identifier(alias)?;
                expr
            }
            _ => {
                return Err(SqlError::Unsupported {
                    feature: "count projection",
                })
            }
        };
        let Expr::Function(function) = expr else {
            return Err(SqlError::Unsupported {
                feature: "COUNT star required",
            });
        };
        if function.name.0.len() != 1
            || function.name.0[0].quote_style.is_some()
            || !function.name.0[0].value.eq_ignore_ascii_case("COUNT")
            || function.parameters != FunctionArguments::None
            || function.filter.is_some()
            || function.null_treatment.is_some()
            || function.over.is_some()
            || !function.within_group.is_empty()
        {
            return Err(SqlError::Unsupported {
                feature: "unqualified builtin COUNT star required",
            });
        }
        let FunctionArguments::List(args) = &function.args else {
            return Err(SqlError::Unsupported {
                feature: "COUNT arguments",
            });
        };
        if args.duplicate_treatment.is_some()
            || !args.clauses.is_empty()
            || !matches!(
                args.args.as_slice(),
                [FunctionArg::Unnamed(FunctionArgExpr::Wildcard)]
            )
        {
            return Err(SqlError::Unsupported {
                feature: "plain COUNT star required",
            });
        }
        let mut guard = query.as_ref().clone();
        let SetExpr::Select(probe) = guard.body.as_mut() else {
            return Err(SqlError::Malformed);
        };
        probe.projection = vec![SelectItem::Wildcard(Default::default())];
        let inner = match &select.from[0].relation {
            TableFactor::Derived {
                lateral: false,
                subquery,
                alias: Some(alias),
            } if select.selection.is_none() => {
                Base::identifier(&alias.name)?;
                if !alias.columns.is_empty() {
                    return Err(SqlError::Unsupported {
                        feature: "count alias columns",
                    });
                }
                // 外层完整形状仍交给同一只读守卫；仅把派生表替换为不执行的虚拟表。
                let dummy = Parser::parse_sql(&dialect, "SELECT * FROM __seata_count_guard")
                    .map_err(|_| SqlError::Malformed)?;
                let Some(Statement::Query(dummy)) = dummy.first() else {
                    return Err(SqlError::Malformed);
                };
                let SetExpr::Select(dummy) = dummy.body.as_ref() else {
                    return Err(SqlError::Malformed);
                };
                probe.from = dummy.from.clone();
                MySqlSelectRecognizer::recognize(&guard.to_string())?;
                MySqlSelectRecognizer::recognize(&subquery.to_string())?
            }
            TableFactor::Table { .. } => MySqlSelectRecognizer::recognize(&guard.to_string())?,
            _ => {
                return Err(SqlError::Unsupported {
                    feature: "count derived shape",
                })
            }
        };
        SqlRecognizerFactory::verify_ast_tokens(&tokens, statement, &dialect)?;
        Ok(Self {
            sql: statement.to_string(),
            inner,
        })
    }
    /// 无参数；返回经完整核对且未增加探测 LIMIT 的统计 SQL。
    pub fn sql(&self) -> &str {
        &self.sql
    }
    /// 无参数；返回真实物理表名。
    pub fn table_name(&self) -> &str {
        self.inner.table_name()
    }
    /// 无参数；返回原序绑定数量。
    pub fn parameter_count(&self) -> usize {
        self.inner.parameter_count()
    }
    /// 参数为完整原绑定；返回原查询/分页参数预算检查，不改变数值或顺序。
    pub fn validate_binding_bounds(&self, values: &[FieldValue]) -> Result<(), SqlError> {
        self.inner.validate_binding_bounds(values)
    }
}
impl fmt::Debug for MySqlCountRecognizer {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("MySqlCountRecognizer")
            .field("table", &self.table_name())
            .field("parameters", &self.parameter_count())
            .finish_non_exhaustive()
    }
}
