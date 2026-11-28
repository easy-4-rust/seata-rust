//! Druid 1.2.28 JDBC URL 分型的原 Java 差分验收。
use seata_sqlparser::{DbTypeParser, DruidDbTypeParserImpl, JavaString};
use std::{collections::BTreeSet, error::Error};

#[test]
fn native_jdbc_parser_matches_original_druid_for_all_prefixes_and_boundaries(
) -> Result<(), Box<dyn Error>> {
    let parser: &dyn DbTypeParser = &DruidDbTypeParserImpl;
    let mut seen = BTreeSet::new();
    for row in include_str!("fixtures/sqlparser_core/jdbc-db-type-java.tsv").lines() {
        let (input, expected) = row.split_once('\t').ok_or("invalid oracle row")?;
        assert!(seen.insert(input), "duplicate input in oracle");
        let units = if input == "null" {
            None
        } else {
            assert!(input.len() % 4 == 0);
            let units = (0..input.len())
                .step_by(4)
                .map(|i| u16::from_str_radix(&input[i..i + 4], 16))
                .collect::<Result<_, _>>()?;
            Some(JavaString::from_utf16(units))
        };
        let actual = parser.parse_from_jdbc_url(units.as_ref())?;
        assert_eq!(
            actual,
            (expected != "null").then(|| JavaString::from(expected)),
            "input UTF-16 units: {input}"
        );
    }
    assert_eq!(seen.len(), 557);
    Ok(())
}
