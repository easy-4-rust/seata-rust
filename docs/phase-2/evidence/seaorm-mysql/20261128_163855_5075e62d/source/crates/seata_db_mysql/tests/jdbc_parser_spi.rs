//! 专用链接域中验证 RM 选择有名、最高优先级的 DbTypeParser。

use seata_db_mysql::JdbcUtils;
use seata_sqlparser::{
    register_db_type_parser, DbTypeParser, DbTypeParserRegistration, JavaString, MetadataError,
    MetadataLocale, SqlParserType,
};
use std::ptr;

struct Parser(&'static str);

impl DbTypeParser for Parser {
    fn parse_from_jdbc_url(
        &self,
        _jdbc_url: Option<&JavaString>,
    ) -> Result<Option<JavaString>, MetadataError> {
        Ok(Some(self.0.into()))
    }
}

static CHOSEN: Parser = Parser("CHOSEN");
static LOWER_PRIORITY: Parser = Parser("LOWER");
static OTHER_NAME: Parser = Parser("OTHER");

register_db_type_parser! {
    DbTypeParserRegistration::new("DRUID", 7, &CHOSEN)
}
register_db_type_parser! {
    DbTypeParserRegistration::new(SqlParserType::SQL_PARSER_TYPE_DRUID, 3, &LOWER_PRIORITY)
}
register_db_type_parser! {
    DbTypeParserRegistration::new(SqlParserType::SQL_PARSER_TYPE_ANTLR, 10, &OTHER_NAME)
}
register_db_type_parser! {
    DbTypeParserRegistration::new("linked-tie", -1, &CHOSEN)
}
register_db_type_parser! {
    DbTypeParserRegistration::new("linked-tie", -1, &LOWER_PRIORITY)
}

#[test]
fn rm_uses_the_highest_order_provider_with_the_druid_name() -> Result<(), MetadataError> {
    let parser = JdbcUtils::get_db_type_parser()?;
    let chosen = ptr::addr_eq(parser, &CHOSEN as &dyn DbTypeParser);
    assert!(chosen);
    let db_type =
        JdbcUtils::get_db_type(Some(&"jdbc:h2:mem:test".into()), &MetadataLocale::new("en"))?;
    assert_eq!(db_type, JavaString::from("chosen"));
    println!("SEATA_JDBC_SPI\toverride\t{chosen}");
    println!("SEATA_JDBC_SPI\trm-type\t{db_type}");
    Ok(())
}

#[test]
fn equal_top_priority_without_a_link_order_is_rejected() {
    assert!(matches!(
        DbTypeParserRegistration::discover(Some(&"linked-tie".into()), &MetadataLocale::new("en")),
        Err(MetadataError::InvalidCompatibilityData(
            "multiple DbTypeParser providers share the highest order"
        ))
    ));
}
