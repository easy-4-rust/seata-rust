//! 原 Java EscapeHandler 默认方法的差分测试。
use seata_sqlparser::{
    EscapeHandler, EscapeSymbol, JavaString, MetadataError, MetadataLocale, TableMeta,
    DEFAULT_ESCAPE_SYMBOL,
};
use std::{cell::RefCell, error::Error, ptr};

#[derive(Default)]
struct Handler<'a> {
    mode: u8,
    required: bool,
    table: Option<&'a TableMeta>,
    trace: RefCell<String>,
}

impl EscapeHandler for Handler<'_> {
    fn check_if_key_words(&self, name: Option<&JavaString>) -> Result<bool, MetadataError> {
        Ok(name.and_then(JavaString::as_str) == Some("select"))
    }

    fn check_if_need_escape(
        &self,
        name: Option<&JavaString>,
        table: Option<&TableMeta>,
    ) -> Result<bool, MetadataError> {
        let same_table = match (self.table, table) {
            (Some(a), Some(b)) => ptr::eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.trace
            .borrow_mut()
            .push_str(&format!("need:{}:{same_table};", hex(name)));
        Ok(self.required)
    }

    fn escape_symbol(&self) -> Result<Option<EscapeSymbol>, MetadataError> {
        self.trace.borrow_mut().push_str("symbol;");
        Ok(Some(match self.mode {
            0 => DEFAULT_ESCAPE_SYMBOL,
            1 => EscapeSymbol::paired(u16::from(b'['), u16::from(b']')),
            _ => EscapeSymbol::new(u16::from(b'.')),
        }))
    }
}

fn hex(value: Option<&JavaString>) -> String {
    let Some(value) = value else {
        return "null".into();
    };
    let digits = b"0123456789abcdef";
    let mut result = String::with_capacity(value.as_utf16().len() * 4);
    for unit in value.as_utf16() {
        for shift in [12, 8, 4, 0] {
            result.push(char::from(digits[usize::from((unit >> shift) & 15)]));
        }
    }
    result
}

fn decode(value: &str) -> Result<Option<JavaString>, Box<dyn Error>> {
    if value == "null" {
        return Ok(None);
    }
    let units = (0..value.len())
        .step_by(4)
        .map(|i| u16::from_str_radix(&value[i..i + 4], 16))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(JavaString::from_utf16(units)))
}

#[test]
fn original_escape_defaults_match_utf16_results_identity_exceptions_and_callbacks(
) -> Result<(), Box<dyn Error>> {
    let table = TableMeta::new(MetadataLocale::new("zh"));
    let mut observations = 0;
    for line in include_str!("fixtures/sqlparser_core/escape-handler-java-v2.tsv").lines() {
        let columns: Vec<_> = line.split('\t').collect();
        if columns[0] == "identity" {
            let handler = Handler {
                required: true,
                ..Handler::default()
            };
            let single = JavaString::from("intern-probe");
            let qualified = JavaString::from("schema.intern-probe");
            for (input, expected) in [(&single, columns[1]), (&qualified, columns[2])] {
                let a = handler.add_col_name_escape(Some(input))?;
                let b = handler.add_col_name_escape(Some(input))?;
                assert_eq!(a.same_storage(&b).to_string(), expected);
            }
        } else {
            let handler = Handler {
                mode: columns[0].parse()?,
                required: columns[1].parse()?,
                table: Some(&table),
                ..Handler::default()
            };
            let name = decode(columns[2])?;
            let input = name.as_ref();
            let result = match columns[3] {
                "contains" => handler.contains_escape(input).map(|v| v.to_string()),
                "add" => handler
                    .add_col_name_escape_with_table(input, Some(&table))
                    .map(|v| encode_result(input, Some(&v))),
                "delete" => handler
                    .del_col_name_escape(input)
                    .map(|v| encode_result(input, v.as_ref())),
                _ => return Err("unknown oracle operation".into()),
            };
            let computed = match result {
                Ok(value) => value,
                Err(error) => {
                    let class = error
                        .java_exception_class()
                        .ok_or("missing exception class")?;
                    if class == "java.lang.StringIndexOutOfBoundsException" {
                        format!("{class}\t{}", hex(error.java_message()))
                    } else {
                        class.into()
                    }
                }
            };
            assert_eq!(
                format!("{computed}\t{}", handler.trace.borrow()),
                columns[4..].join("\t"),
                "{line:?}"
            );
        }
        observations += 1;
    }
    assert_eq!(observations, 397);
    Ok(())
}

fn encode_result(input: Option<&JavaString>, output: Option<&JavaString>) -> String {
    let same = match (input, output) {
        (Some(a), Some(b)) => a.same_storage(b),
        (None, None) => true,
        _ => false,
    };
    format!("{}\t{same}", hex(output))
}
