use crate::{mysql::BaseMySqlRecognizer as Base, SqlError, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use sqlparser::{
    ast::{Expr, GroupByExpr, OffsetRows, SelectItem, SetExpr, Statement, Value},
    dialect::MySqlDialect,
    parser::Parser,
};
use std::{collections::HashSet, fmt};

/// 有界、无副作用的单表快照查询计划；不属于写 SQL 或 Undo 类型。
/// 对应 Java: 无直接对象；SelectForUpdateExecutor 之外的 Rust ORM 普通读取准入扩展。
pub struct MySqlSelectRecognizer {
    table: String,
    columns: Vec<(String, String)>,
    wildcard: bool,
    suffix: String,
    parameters: usize,
    bounds: Vec<(usize, u64)>,
    qualifier: String,
    pagination_index: usize,
    ordered: bool,
}
impl MySqlSelectRecognizer {
    /// 参数为原 SELECT；返回已完整核对单语句/表达式/分页的密封计划，不开放原 SQL 执行能力。
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
                reason: "exactly one SELECT required",
            });
        };
        let Statement::Query(query) = statement else {
            return Err(SqlError::Unsupported {
                feature: "non SELECT read",
            });
        };
        if query.with.is_some()
            || !query.limit_by.is_empty()
            || query.fetch.is_some()
            || !query.locks.is_empty()
            || query.for_clause.is_some()
            || query.settings.is_some()
            || query.format_clause.is_some()
            || (query.offset.is_some() && query.limit.is_none())
        {
            return Err(SqlError::Unsupported {
                feature: "read CTE/locking/extended pagination",
            });
        }
        let SetExpr::Select(select) = query.body.as_ref() else {
            return Err(SqlError::Unsupported {
                feature: "read set/subquery",
            });
        };
        if select.distinct.is_some()
            || select.top.is_some()
            || select.into.is_some()
            || select.from.len() != 1
            || !select.lateral_views.is_empty()
            || select.prewhere.is_some()
            || !matches!(&select.group_by,GroupByExpr::Expressions(v,m) if v.is_empty() && m.is_empty())
            || !select.cluster_by.is_empty()
            || !select.distribute_by.is_empty()
            || !select.sort_by.is_empty()
            || select.having.is_some()
            || !select.named_window.is_empty()
            || select.qualify.is_some()
            || select.value_table_mode.is_some()
            || select.connect_by.is_some()
        {
            return Err(SqlError::Unsupported {
                feature: "read projection/aggregation/INTO shape",
            });
        }
        let (table, alias) = Base::table(&select.from[0])?;
        let qualifier = alias.as_deref().unwrap_or(&table);
        if select.projection.is_empty() || select.projection.len() > 128 {
            return Err(SqlError::Budget {
                limit: "1..128 read columns",
            });
        }
        let mut columns = Vec::new();
        let mut wildcard = false;
        let mut labels = HashSet::new();
        for item in &select.projection {
            let pair = match item {
                SelectItem::UnnamedExpr(expr) => {
                    let name = Self::column(expr, qualifier)?;
                    (name.clone(), name)
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    (Self::column(expr, qualifier)?, Base::identifier(alias)?)
                }
                SelectItem::Wildcard(options)
                    if select.projection.len() == 1 && *options == Default::default() =>
                {
                    wildcard = true;
                    continue;
                }
                SelectItem::QualifiedWildcard(owner, options)
                    if select.projection.len() == 1
                        && *options == Default::default()
                        && Base::object(owner)? == qualifier =>
                {
                    wildcard = true;
                    continue;
                }
                _ => {
                    return Err(SqlError::Unsupported {
                        feature: "read expression/function/wildcard options",
                    })
                }
            };
            if !labels.insert(pair.1.to_ascii_lowercase()) {
                return Err(SqlError::Invalid {
                    reason: "duplicate read label",
                });
            }
            columns.push(pair);
        }
        let mut parameters = 0;
        let mut bounds = Vec::new();
        let predicate = Base::predicate(&select.selection, &mut parameters, qualifier)?;
        let mut suffix = format!(" FROM {}", select.from[0]);
        if let Some(predicate) = predicate {
            suffix.push_str(&format!(" WHERE {predicate}"));
        }
        if let Some(order) = &query.order_by {
            if order.exprs.len() > 16 || order.interpolate.is_some() {
                return Err(SqlError::Unsupported {
                    feature: "read order shape",
                });
            }
            for value in &order.exprs {
                if matches!(&value.expr,Expr::Identifier(name) if columns.iter().any(|(source,label)| label.eq_ignore_ascii_case(&name.value) && !source.eq_ignore_ascii_case(label)))
                {
                    return Err(SqlError::Unsupported {
                        feature: "read order by renamed projection",
                    });
                }
                Self::column(&value.expr, qualifier)?;
                if value.nulls_first.is_some() || value.with_fill.is_some() {
                    return Err(SqlError::Unsupported {
                        feature: "read order options",
                    });
                }
            }
            suffix.push_str(&format!(" {order}"));
        }
        let pagination_index = suffix.len();
        if let Some(limit) = &query.limit {
            Self::page(limit, 1024, &mut parameters, &mut bounds)?;
            suffix.push_str(&format!(" LIMIT {limit}"));
        } else {
            suffix.push_str(" LIMIT 1025");
        }
        if let Some(offset) = &query.offset {
            if offset.rows != OffsetRows::None {
                return Err(SqlError::Unsupported {
                    feature: "read offset rows",
                });
            }
            Self::page(&offset.value, 1_000_000, &mut parameters, &mut bounds)?;
            suffix.push_str(&format!(" {offset}"));
        }
        SqlRecognizerFactory::verify_ast_tokens(&tokens, statement, &dialect)?;
        Ok(Self {
            qualifier: qualifier.to_owned(),
            table,
            columns,
            wildcard,
            suffix,
            parameters,
            bounds,
            pagination_index,
            ordered: query.order_by.is_some(),
        })
    }
    fn column(expr: &Expr, qualifier: &str) -> Result<String, SqlError> {
        match expr {
            Expr::Identifier(value) => {
                Base::expression(expr, &mut 0, 0, Some(qualifier))?;
                Base::identifier(value)
            }
            Expr::CompoundIdentifier(parts) => Base::column(parts, qualifier),
            _ => Err(SqlError::Unsupported {
                feature: "direct read column required",
            }),
        }
    }
    fn page(
        expr: &Expr,
        max: u64,
        parameters: &mut usize,
        bounds: &mut Vec<(usize, u64)>,
    ) -> Result<(), SqlError> {
        match expr {
            Expr::Value(Value::Number(value, false))
                if value.parse::<u64>().is_ok_and(|value| value <= max) =>
            {
                Ok(())
            }
            Expr::Value(Value::Placeholder(value)) if value == "?" => {
                bounds.push((*parameters, max));
                *parameters += 1;
                Ok(())
            }
            _ => Err(SqlError::Budget {
                limit: "read pagination",
            }),
        }
    }
    /// 参数为完整原绑定；返回参数数量及分页整数范围检查，不改变绑定顺序或数值。
    pub fn validate_binding_bounds(&self, values: &[FieldValue]) -> Result<(), SqlError> {
        if values.len() != self.parameters || values.len() > 1024 {
            return Err(SqlError::Invalid {
                reason: "exact read parameter count",
            });
        }
        for (index, max) in &self.bounds {
            let value = match values.get(*index) {
                Some(FieldValue::Unsigned(v)) => *v,
                Some(FieldValue::Signed(v)) if *v >= 0 => *v as u64,
                _ => {
                    return Err(SqlError::Invalid {
                        reason: "unsigned read pagination parameter",
                    })
                }
            };
            if value > *max {
                return Err(SqlError::Budget {
                    limit: "read pagination parameter",
                });
            }
        }
        Ok(())
    }
    /// 参数为同一真实表元数据；返回保留排序语义并用完整主键稳定并列行的安全后缀。
    pub fn stable_suffix_sql(
        &self,
        table: &seata_datasource::TableMeta,
    ) -> Result<String, SqlError> {
        if table.name() != self.table {
            return Err(SqlError::Invalid {
                reason: "read metadata table differs",
            });
        }
        let owner = format!("`{}`", self.qualifier);
        let keys = table
            .primary_key_columns()
            .map(|column| format!("{owner}.`{}`", column.name()))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(format!(
            "{}{}{}{}",
            &self.suffix[..self.pagination_index],
            if self.ordered { ", " } else { " ORDER BY " },
            keys,
            &self.suffix[self.pagination_index..]
        ))
    }
    /// 无参数；返回当前合法表限定名称，用于真实列投影。
    pub fn qualifier(&self) -> &str {
        &self.qualifier
    }
    /// 无参数；返回真实单表名称，不含 schema 或别名。
    pub fn table_name(&self) -> &str {
        &self.table
    }
    /// 无参数；按原投影顺序返回真实列名与结果标签，通配符时为空。
    pub fn columns(&self) -> &[(String, String)] {
        &self.columns
    }
    /// 无参数；返回是否由已核对的单独通配符选择物理完整列序。
    pub fn is_wildcard(&self) -> bool {
        self.wildcard
    }
    /// 无参数；返回已密封且有界的 FROM/WHERE/ORDER/LIMIT/OFFSET 后缀。
    pub fn suffix_sql(&self) -> &str {
        &self.suffix
    }
    /// 无参数；返回原占位符数量，包含分页绑定。
    pub fn parameter_count(&self) -> usize {
        self.parameters
    }
}
impl fmt::Debug for MySqlSelectRecognizer {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("MySqlSelectRecognizer")
            .field("table", &self.table)
            .field("columns", &self.columns.len())
            .field("parameters", &self.parameters)
            .finish_non_exhaustive()
    }
}
