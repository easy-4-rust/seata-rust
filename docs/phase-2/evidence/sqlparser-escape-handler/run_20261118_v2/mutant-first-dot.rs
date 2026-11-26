use crate::{EscapeSymbol, JavaObjectError, JavaString, MetadataError, TableMeta};
use std::sync::Arc;

/// 标识符的默认双引号。
/// 对应 Java: EscapeHandler.DEFAULT_ESCAPE_SYMBOL。
pub const DEFAULT_ESCAPE_SYMBOL: EscapeSymbol = EscapeSymbol::new(b'"' as u16);

/// 标识符限定符分隔符。
/// 对应 Java: EscapeHandler.DOT。
pub const DOT: u16 = b'.' as u16;

/// 数据库关键字识别和 UTF-16 标识符转义。
/// 对应 Java: org.apache.seata.sqlparser.EscapeHandler。
pub trait EscapeHandler {
    /// 检查可空 name 是否为该数据库的关键字，返回判定或原回调错误。
    /// 对应 Java: EscapeHandler#checkIfKeyWords。
    fn check_if_key_words(&self, name: Option<&JavaString>) -> Result<bool, MetadataError>;

    /// 针对可空 name 和 table 执行方言特有的转义判定，返回判定或原回调错误。
    /// 对应 Java: EscapeHandler#checkIfNeedEscape。
    fn check_if_need_escape(
        &self,
        name: Option<&JavaString>,
        table: Option<&TableMeta>,
    ) -> Result<bool, MetadataError>;

    /// 返回默认双引号，方言可返回自定义符号、空引用或原回调错误。
    /// 对应 Java: EscapeHandler#getEscapeSymbol。
    fn escape_symbol(&self) -> Result<Option<EscapeSymbol>, MetadataError> {
        Ok(Some(DEFAULT_ESCAPE_SYMBOL))
    }

    /// 去除 name 两端不大于 U+0020 的码元，检查左右转义符。
    /// 对应 Java: EscapeHandler#containsEscape，空引用或裁剪后的空串保留原异常。
    fn contains_escape(&self, name: Option<&JavaString>) -> Result<bool, MetadataError> {
        let name = name.ok_or(MetadataError::NullReference("fieldOrTableName"))?;
        let units = java_trim(name.as_utf16());
        let symbol = self.escape_symbol()?;
        let first = units
            .first()
            .ok_or_else(|| string_bounds("String index out of range: 0".into()))?;
        let symbol = symbol.ok_or(MetadataError::NullReference("escapeSymbol"))?;
        Ok(*first == symbol.left_symbol() && units.last() == Some(&symbol.right_symbol()))
    }

    /// 用无表上下文的方言规则转义可空 col_name，返回名称或原错误。
    /// 对应 Java: EscapeHandler#addColNameEscape(String)。
    fn add_col_name_escape(
        &self,
        col_name: Option<&JavaString>,
    ) -> Result<JavaString, MetadataError> {
        self.add_col_name_escape_with_table(col_name, None)
    }

    /// 用可空 table 判定最后一段名称，再按首个匹配分隔形式拼接整个 col_name。
    /// 对应 Java: EscapeHandler#addColNameEscape(String, TableMeta)。
    fn add_col_name_escape_with_table(
        &self,
        col_name: Option<&JavaString>,
        table: Option<&TableMeta>,
    ) -> Result<JavaString, MetadataError> {
        let col_name = col_name.ok_or(MetadataError::NullReference("colName"))?;
        let units = col_name.as_utf16();
        let last_dot = units.iter().position(|&unit| unit == DOT);
        let suffix = last_dot.map(|index| JavaString::from_utf16(units[index + 1..].to_vec()));
        if !self.check_if_need_escape(Some(suffix.as_ref().unwrap_or(col_name)), table)? {
            return Ok(col_name.clone());
        }
        let symbol = self
            .escape_symbol()?
            .ok_or(MetadataError::NullReference("escapeChar"))?;
        let left = symbol.left_symbol();
        let right = symbol.right_symbol();
        if last_dot.is_some() {
            if let Some(end) = find(units, &[right, DOT]) {
                let split = end + 2;
                return Ok(concat(&[
                    &units[..split],
                    &[left],
                    &units[split..],
                    &[right],
                ]));
            }
            if let Some(split) = find(units, &[DOT, left]) {
                return Ok(concat(&[
                    &[left],
                    &units[..split],
                    &[right],
                    &units[split..],
                ]));
            }
            if let Some(split) = find(units, &[DOT]) {
                return Ok(concat(&[
                    &[left],
                    &units[..split],
                    &[right, DOT, left],
                    &units[split + 1..],
                    &[right],
                ]));
            }
        }
        JavaString::intern_utf16([&[left][..], units, &[right]].concat())
    }

    /// 删除 col_name 中原规则识别的转义符；空白和空引用原样返回。
    /// 对应 Java: EscapeHandler#delColNameEscape。
    fn del_col_name_escape(
        &self,
        col_name: Option<&JavaString>,
    ) -> Result<Option<JavaString>, MetadataError> {
        let Some(col_name) = col_name.filter(|name| !is_java_blank(name)) else {
            return Ok(col_name.cloned());
        };
        let units = col_name.as_utf16();
        let symbol = self
            .escape_symbol()?
            .ok_or(MetadataError::NullReference("escapeChar"))?;
        let left = symbol.left_symbol();
        let right = symbol.right_symbol();
        let starts = units.first() == Some(&left);
        let ends = units.last() == Some(&right);
        let last = units.len() - 1;
        // 原实现按固定顺序处理双侧、仅左侧、仅右侧转义。
        let result = if starts && ends {
            if let Some(index) = find(units, &[right, DOT, left]) {
                concat(&[
                    slice(units, 1, index)?,
                    &[DOT],
                    slice(units, index + 3, last)?,
                ])
            } else {
                JavaString::from_utf16(slice(units, 1, last)?.to_vec())
            }
        } else if let Some(index) = find(units, &[right, DOT]).filter(|_| starts) {
            concat(&[slice(units, 1, index)?, &[DOT], &units[index + 2..]])
        } else if let Some(index) = find(units, &[DOT, left]).filter(|_| ends) {
            concat(&[&units[..index], &[DOT], slice(units, index + 2, last)?])
        } else {
            col_name.clone()
        };
        Ok(Some(result))
    }
}

/// 按 Java UTF-16 char 的 Character.isWhitespace 判断 name 是否全空白。
/// 对应 Java: org.apache.seata.common.util.StringUtils#isBlank。
pub(crate) fn is_java_blank(name: &JavaString) -> bool {
    name.as_utf16().iter().all(|unit| {
        matches!(unit,
            0x0009..=0x000d | 0x001c..=0x0020 | 0x1680 |
            0x2000..=0x2006 | 0x2008..=0x200a | 0x2028..=0x2029 | 0x205f | 0x3000
        )
    })
}

fn find(units: &[u16], pattern: &[u16]) -> Option<usize> {
    units
        .windows(pattern.len())
        .position(|window| window == pattern)
}

fn concat(parts: &[&[u16]]) -> JavaString {
    JavaString::from_utf16(parts.concat())
}

fn slice(units: &[u16], start: usize, end: usize) -> Result<&[u16], MetadataError> {
    units.get(start..end).ok_or_else(|| {
        string_bounds(format!("begin {start}, end {end}, length {}", units.len()).into())
    })
}

fn string_bounds(message: JavaString) -> MetadataError {
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::new("java.lang.StringIndexOutOfBoundsException", Some(message))
            .with_superclasses([
                "java.lang.IndexOutOfBoundsException",
                "java.lang.RuntimeException",
                "java.lang.Exception",
                "java.lang.Throwable",
            ]),
    ))
}

fn java_trim(units: &[u16]) -> &[u16] {
    let Some(start) = units.iter().position(|&unit| unit > 0x20) else {
        return &[];
    };
    let end = units.iter().rposition(|&unit| unit > 0x20).unwrap_or(start);
    &units[start..=end]
}
