//! 固定版 Java rm-datasource 的 JdbcUtilsTest 逐案例迁移。

use seata_db_mysql::JdbcUtils;
use seata_sqlparser::{
    DbTypeParserRegistration, JavaString, MetadataError, MetadataLocale, SqlParserType,
};
use std::ptr;

const MYSQL_URL: &str = "jdbc:mysql://localhost:3306/test";

#[test]
fn test_db_type_parser_loading() -> Result<(), MetadataError> {
    JdbcUtils::get_db_type_parser()?;
    println!("SEATA_JDBC_SOURCE\ttestDbTypeParserLoading\ttrue");
    Ok(())
}

#[test]
fn test_db_type_parser_singleton_pattern() -> Result<(), MetadataError> {
    let first = JdbcUtils::get_db_type_parser()?;
    let second = JdbcUtils::get_db_type_parser()?;
    let same = ptr::addr_eq(first, second);
    assert!(same);
    println!("SEATA_JDBC_SOURCE\ttestDbTypeParserSingletonPattern\t{same}");
    Ok(())
}

#[test]
fn default_parser_is_shared_with_the_registry_and_missing_name_keeps_java_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let parser = JdbcUtils::get_db_type_parser()?;
    let locale = MetadataLocale::new("en");
    let linked = DbTypeParserRegistration::discover(Some(&"DRUID".into()), &locale)?;
    let shared = ptr::addr_eq(parser, linked);
    assert!(shared);
    let Err(missing) = DbTypeParserRegistration::discover(
        Some(&SqlParserType::SQL_PARSER_TYPE_ANTLR.into()),
        &locale,
    ) else {
        panic!("Unexpected ANTLR provider");
    };
    let class = missing
        .java_exception_class()
        .ok_or("missing exception class")?;
    let message = missing.java_message().ok_or("missing exception message")?;
    assert_eq!(
        class,
        "org.apache.seata.common.loader.EnhancedServiceNotFoundException"
    );
    assert_eq!(
        *message,
        JavaString::from(
            "not found service provider for : org.apache.seata.sqlparser.util.DbTypeParser"
        )
    );
    println!("SEATA_JDBC_SPI\tshared\t{shared}");
    println!("SEATA_JDBC_SPI\tmissing\t{class}\t{message}");
    let Err(invalid) = DbTypeParserRegistration::discover(Some(&"".into()), &locale) else {
        panic!("Unexpected empty-name provider");
    };
    let class = invalid
        .java_exception_class()
        .ok_or("missing exception class")?;
    let message = invalid.java_message().ok_or("missing exception message")?;
    assert_eq!(class, "java.lang.IllegalArgumentException");
    assert_eq!(*message, JavaString::from("the name of service provider for [org.apache.seata.sqlparser.util.DbTypeParser] name is null"));
    println!("SEATA_JDBC_SPI\tempty\t{class}\t{message}");
    Ok(())
}

fn check_db_type(case: &str, url: &str, expected: &str) {
    let result = JdbcUtils::get_db_type(Some(&JavaString::from(url)), &MetadataLocale::new("en"));
    check_observation(case, result, expected);
}

fn check_resource_id(case: &str, url: &str) {
    let result = JdbcUtils::build_resource_id(Some(&JavaString::from(url)));
    check_observation(case, result, MYSQL_URL);
}

fn check_observation(case: &str, result: Result<JavaString, MetadataError>, expected: &str) {
    let Ok(actual) = result else {
        panic!("{case}: {result:?}");
    };
    assert_eq!(actual, JavaString::from(expected), "{case}");
    println!("SEATA_JDBC_SOURCE\t{case}\t{actual}");
}

#[test]
fn test_get_db_type_with_mysql_url() {
    check_db_type("testGetDbTypeWithMySQLUrl", MYSQL_URL, "mysql");
}

#[test]
fn test_get_db_type_with_oracle_url() {
    check_db_type(
        "testGetDbTypeWithOracleUrl",
        "jdbc:oracle:thin:@localhost:1521:orcl",
        "oracle",
    );
}

#[test]
fn test_get_db_type_with_postgresql_url() {
    check_db_type(
        "testGetDbTypeWithPostgreSQLUrl",
        "jdbc:postgresql://localhost:5432/test",
        "postgresql",
    );
}

#[test]
fn test_get_db_type_with_h2_url() {
    check_db_type("testGetDbTypeWithH2Url", "jdbc:h2:mem:test", "h2");
}

#[test]
fn test_build_resource_id_with_query_string() {
    check_resource_id(
        "testBuildResourceIdWithQueryString",
        "jdbc:mysql://localhost:3306/test?useSSL=false&serverTimezone=UTC",
    );
}

#[test]
fn test_build_resource_id_without_query_string() {
    check_resource_id("testBuildResourceIdWithoutQueryString", MYSQL_URL);
}

#[test]
fn test_build_resource_id_with_empty_query_string() {
    check_resource_id(
        "testBuildResourceIdWithEmptyQueryString",
        "jdbc:mysql://localhost:3306/test?",
    );
}

#[test]
fn test_build_resource_id_with_multiple_question_marks() {
    check_resource_id(
        "testBuildResourceIdWithMultipleQuestionMarks",
        "jdbc:mysql://localhost:3306/test?param1=value1?param2=value2",
    );
}
