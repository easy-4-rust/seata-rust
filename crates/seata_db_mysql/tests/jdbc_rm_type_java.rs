//! 原 Java RM JDBC 类型入口的差分验收。
use seata_db_mysql::JdbcUtils;
use seata_sqlparser::{JavaString, MetadataLocale};
use std::{collections::BTreeSet, error::Error};

#[test]
fn rm_jdbc_url_type_keeps_locale_fold_and_original_null_error() -> Result<(), Box<dyn Error>> {
    let locale = MetadataLocale::new("en");
    let mut seen = BTreeSet::new();
    for row in include_str!("fixtures/jdbc-rm-db-type-java.tsv").lines() {
        let (input, expected) = row.split_once('\t').ok_or("invalid oracle row")?;
        assert!(seen.insert(input), "duplicate Java observation");
        let value = if input == "null" {
            None
        } else {
            assert!(input.len() % 4 == 0);
            let units = (0..input.len())
                .step_by(4)
                .map(|i| u16::from_str_radix(&input[i..i + 4], 16))
                .collect::<Result<_, _>>()?;
            Some(JavaString::from_utf16(units))
        };
        match JdbcUtils::get_db_type(value.as_ref(), &locale) {
            Ok(result) => assert_eq!(result, JavaString::from(expected), "{input}"),
            Err(error) => assert_eq!(error.java_exception_class(), Some(expected), "{input}"),
        }
    }
    assert_eq!(seen.len(), 557);
    Ok(())
}
