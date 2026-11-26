use seata_sqlparser::{
    EscapeHandler, EscapeSymbol, JavaString, MetadataError, MetadataLocale, TableMeta,
};
use std::{collections::HashSet, sync::OnceLock};

/// 默认引号方言在普通标识符上的大小写规则。
pub(crate) enum CaseRule {
    Uppercase,
    UppercaseUnlessTablePresent,
    LowercaseUnlessTablePresent,
    KeywordsOnly,
}

/// 多个原 Java 方言共享的关键字和列元数据查找路径。
/// 对应 Java: Dm、Oracle、OceanBase、Oscar、Kingbase、Postgresql、SqlServer 的原处理规则。
pub(crate) struct DialectEscapeRules {
    locale: MetadataLocale,
    keywords: &'static HashSet<&'static str>,
    case_rule: CaseRule,
    symbol: EscapeSymbol,
}

impl DialectEscapeRules {
    /// 接收语言、原关键字定义、大小写规则及转义符，构造供方言使用的行为对象。
    /// 对应 Java: 各具体 EscapeHandler 的初始化。
    pub(crate) fn new(
        locale: MetadataLocale,
        keywords: &'static OnceLock<HashSet<&'static str>>,
        input: &'static str,
        case_rule: CaseRule,
        symbol: EscapeSymbol,
    ) -> Self {
        Self {
            locale,
            keywords: keywords.get_or_init(|| input.lines().collect()),
            case_rule,
            symbol,
        }
    }
}

impl EscapeHandler for DialectEscapeRules {
    /// 判断原始名称或按当前 Locale 转大写后的名称是否在方言关键字表内。
    /// 对应 Java: 各具体 EscapeHandler#checkIfKeyWords。
    fn check_if_key_words(&self, name: Option<&JavaString>) -> Result<bool, MetadataError> {
        let Some(name) = name else {
            return Ok(false);
        };
        if name.as_str().is_some_and(|v| self.keywords.contains(v)) {
            return Ok(true);
        }
        Ok(self
            .locale
            .upper(name)?
            .as_str()
            .is_some_and(|v| self.keywords.contains(v)))
    }

    /// 判断名称是否需要转义；元数据中的同名列优先于普通名称大小写规则。
    /// 对应 Java: 各具体 EscapeHandler#checkIfNeedEscape。
    fn check_if_need_escape(
        &self,
        name: Option<&JavaString>,
        table: Option<&TableMeta>,
    ) -> Result<bool, MetadataError> {
        if matches!(self.case_rule, CaseRule::KeywordsOnly) {
            return self.check_if_key_words(name);
        }
        let Some(name) = name.filter(|value| !value.is_java_blank()) else {
            return Ok(false);
        };
        let name = name.java_trim();
        if self.contains_escape(Some(&name))? {
            return Ok(false);
        }
        if self.check_if_key_words(Some(&name))? {
            return Ok(true);
        }
        if let Some(table) = table {
            if let Some(column) = table.column_meta(Some(&name))? {
                return column.is_case_sensitive();
            }
            if !matches!(self.case_rule, CaseRule::Uppercase) {
                return Ok(true);
            }
        }
        let (lower, upper) = ('a' as u16..='z' as u16, 'A' as u16..='Z' as u16);
        Ok(match self.case_rule {
            CaseRule::LowercaseUnlessTablePresent => {
                name.as_utf16().iter().any(|unit| upper.contains(unit))
            }
            _ => name.as_utf16().iter().any(|unit| lower.contains(unit)),
        })
    }

    /// 返回构造时绑定的转义符。
    /// 对应 Java: 各具体 EscapeHandler#getEscapeSymbol。
    fn escape_symbol(&self) -> Result<Option<EscapeSymbol>, MetadataError> {
        Ok(Some(self.symbol))
    }
}

macro_rules! delegate_escape_handler {
    ($provider:ty) => {
        impl seata_sqlparser::EscapeHandler for $provider {
            /// 使用该方言的关键字表和 Locale 判断可空名称。
            /// 对应 Java: EscapeHandler#checkIfKeyWords。
            fn check_if_key_words(
                &self,
                name: Option<&seata_sqlparser::JavaString>,
            ) -> Result<bool, seata_sqlparser::MetadataError> {
                self.inner.check_if_key_words(name)
            }

            /// 使用该方言规则判定可空名称是否需要转义，必要时读取表元数据。
            /// 对应 Java: EscapeHandler#checkIfNeedEscape。
            fn check_if_need_escape(
                &self,
                name: Option<&seata_sqlparser::JavaString>,
                table: Option<&seata_sqlparser::TableMeta>,
            ) -> Result<bool, seata_sqlparser::MetadataError> {
                self.inner.check_if_need_escape(name, table)
            }

            /// 返回该方言的转义符。
            /// 对应 Java: EscapeHandler#getEscapeSymbol。
            fn escape_symbol(
                &self,
            ) -> Result<Option<seata_sqlparser::EscapeSymbol>, seata_sqlparser::MetadataError> {
                self.inner.escape_symbol()
            }
        }
    };
}

pub(crate) use delegate_escape_handler;
