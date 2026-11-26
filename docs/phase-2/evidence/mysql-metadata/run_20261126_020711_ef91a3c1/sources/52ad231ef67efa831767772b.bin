use crate::mysql::{MySqlDeleteRecognizer, MySqlInsertRecognizer, MySqlUpdateRecognizer};
use crate::{SqlError, SqlRecognizer};
use sqlparser::ast::Statement;
use sqlparser::dialect::MySqlDialect;
use sqlparser::keywords::Keyword;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, Tokenizer, Whitespace};

/// 有界MySQL AT工厂；完整验证唯一语句后才暴露识别结果，不参与业务执行。
/// 对应 Java: org.apache.seata.sqlparser.SQLRecognizerFactory；显式选择当前Rust/MySQL支持矩阵。
pub struct SqlRecognizerFactory;
impl SqlRecognizerFactory {
    /// 参数为原SQL文本；返回完整已验证识别结果。超限、未支持语法与格式错误显式区分。
    /// 字面值不进入错误或Debug；原SQL仍保留供绑定后执行，数据库元数据检查不得省略。
    pub fn recognize(sql: &str) -> Result<SqlRecognizer, SqlError> {
        let dialect = MySqlDialect {};
        let tokens = Self::checked_tokens(sql, &dialect)?;
        let statements = Parser::new(&dialect)
            .with_recursion_limit(64)
            .with_tokens(tokens.clone())
            .parse_statements()
            .map_err(|_| SqlError::Malformed)?;
        if statements.len() != 1 {
            return Err(SqlError::Invalid {
                reason: "exactly one statement required",
            });
        }
        let mut result = match &statements[0] {
            Statement::Insert(value) => MySqlInsertRecognizer::recognize(value),
            Statement::Update { .. } => MySqlUpdateRecognizer::recognize(&statements[0]),
            Statement::Delete(value) => MySqlDeleteRecognizer::recognize(value),
            _ => Err(SqlError::Unsupported {
                feature: "non INSERT/UPDATE/DELETE statement",
            }),
        }?;
        Self::verify_ast_tokens(&tokens, &statements[0], &dialect)?;
        result.sql = sql.to_owned();
        Ok(result)
    }
    fn checked_tokens(sql: &str, dialect: &MySqlDialect) -> Result<Vec<Token>, SqlError> {
        if sql.len() > 65_536 {
            return Err(SqlError::Budget { limit: "SQL bytes" });
        }
        if sql.is_empty() || sql.bytes().any(|b| b == 0 || b == b'\\') {
            return Err(SqlError::Invalid {
                reason: "empty/NUL/backslash SQL",
            });
        }
        let tokens = Tokenizer::new(dialect, sql)
            .tokenize()
            .map_err(|_| SqlError::Malformed)?;
        if tokens.iter().any(|token| {
            matches!(
                token,
                Token::Whitespace(
                    Whitespace::SingleLineComment { .. } | Whitespace::MultiLineComment(_)
                )
            )
        }) {
            return Err(SqlError::Unsupported {
                feature: "SQL comments/conditional comments/hints",
            });
        }
        Self::check_tokens(&tokens)?;
        Ok(tokens
            .into_iter()
            .filter(|token| !matches!(token, Token::Whitespace(_)))
            .collect())
    }
    fn verify_ast_tokens(
        original: &[Token],
        statement: &Statement,
        dialect: &MySqlDialect,
    ) -> Result<(), SqlError> {
        let canonical = Tokenizer::new(dialect, &statement.to_string())
            .tokenize()
            .map_err(|_| SqlError::Malformed)?;
        let canonical: Vec<_> = canonical
            .iter()
            .filter(|token| !matches!(token, Token::Whitespace(_)))
            .collect();
        let original = if matches!(original.last(), Some(Token::SemiColon)) {
            &original[..original.len() - 1]
        } else {
            original
        };
        let equal = original.len() == canonical.len()
            && original
                .iter()
                .zip(canonical)
                .all(|(left, right)| match (left, right) {
                    (Token::Word(a), Token::Word(b))
                        if a.keyword != Keyword::NoKeyword
                            && b.keyword != Keyword::NoKeyword
                            && a.quote_style.is_none()
                            && b.quote_style.is_none() =>
                    {
                        a.keyword == b.keyword && a.value.eq_ignore_ascii_case(&b.value)
                    }
                    _ => left == right,
                });
        if !equal {
            return Err(SqlError::Unsupported {
                feature: "parser normalized/discarded SQL tokens",
            });
        }
        Ok(())
    }
    fn check_tokens(tokens: &[Token]) -> Result<(), SqlError> {
        let significant: Vec<_> = tokens
            .iter()
            .filter(|token| !matches!(token, Token::Whitespace(_)))
            .collect();
        if !matches!(significant.first(), Some(Token::Word(word)) if matches!(word.keyword, Keyword::INSERT | Keyword::UPDATE | Keyword::DELETE))
        {
            return Err(SqlError::Unsupported {
                feature: "non-DML first verb",
            });
        }
        // 固定MySQL方言的DIV分支内部unwrap可能对坏输入panic；该运算本轮未支持，先拒绝。
        if significant
            .iter()
            .any(|token| matches!(token, Token::Word(word) if word.keyword == Keyword::DIV))
        {
            return Err(SqlError::Unsupported {
                feature: "MySQL DIV operator",
            });
        }
        if significant.len() > 8192 {
            return Err(SqlError::Budget {
                limit: "SQL tokens",
            });
        }
        // Pratt解析器可为扁平长链构造很深的左树；必须在AST分配和递归析构前限制。
        let mut operators = 0;
        let mut nesting = 0usize;
        for token in &significant {
            match token {
                Token::LParen => {
                    nesting += 1;
                    if nesting > 64 {
                        return Err(SqlError::Budget {
                            limit: "SQL parentheses",
                        });
                    }
                }
                Token::RParen => {
                    nesting = nesting.checked_sub(1).ok_or(SqlError::Malformed)?;
                }
                Token::Word(word)
                    if matches!(
                        word.keyword,
                        Keyword::NoKeyword
                            | Keyword::INSERT
                            | Keyword::INTO
                            | Keyword::VALUES
                            | Keyword::UPDATE
                            | Keyword::SET
                            | Keyword::DELETE
                            | Keyword::FROM
                            | Keyword::WHERE
                            | Keyword::NULL
                            | Keyword::TRUE
                            | Keyword::FALSE
                    ) => {}
                Token::Number(_, _)
                | Token::SingleQuotedString(_)
                | Token::DoubleQuotedString(_)
                | Token::Placeholder(_)
                | Token::Comma
                | Token::SemiColon => {}
                // 未支持的运算符也可能在解析器中形成深树，不能只计算AT白名单运算符。
                _ => operators += 1,
            }
            if operators > 256 {
                return Err(SqlError::Budget {
                    limit: "SQL operators",
                });
            }
        }
        if nesting != 0 {
            return Err(SqlError::Malformed);
        }
        let update = matches!(significant.first(), Some(Token::Word(word)) if word.keyword == Keyword::UPDATE);
        if update
            && significant
                .iter()
                .any(|token| matches!(token, Token::Word(word) if word.keyword == Keyword::FROM))
        {
            return Err(SqlError::Unsupported {
                feature: "UPDATE FROM",
            });
        }
        let terminators = significant
            .iter()
            .filter(|token| matches!(token, Token::SemiColon))
            .count();
        if terminators > 1
            || (terminators == 1 && !matches!(significant.last(), Some(Token::SemiColon)))
        {
            return Err(SqlError::Unsupported {
                feature: "multiple statements/terminators",
            });
        }
        Ok(())
    }
}
