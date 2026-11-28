//! 对固定原 Java 运行时导出的完整常量表做逐项比较；不计为原测试案例。
use seata_sqlparser::{IndexType, JdbcConstants, SQLType, SqlParserType};

#[test]
fn complete_constant_inventory_matches_original_java() {
    let mut actual: Vec<String> = SQLType::ALL
        .iter()
        .map(|value| format!("SQLType\t{value}\t{}", value.value()))
        .chain(
            IndexType::ALL
                .iter()
                .map(|value| format!("IndexType\t{value}\t{}", value.value())),
        )
        .collect();
    macro_rules! jdbc {
        ($($name:ident),+ $(,)?) => {
            $(actual.push(format!("JdbcConstants\t{}\t{}", stringify!($name), JdbcConstants::$name));)+
        };
    }
    jdbc!(
        ORACLE,
        MYSQL,
        DB2,
        H2,
        MARIADB,
        POSTGRESQL,
        SQLSERVER,
        JTDS,
        HSQL,
        SYBASE,
        DERBY,
        HBASE,
        HIVE,
        DM,
        KINGBASE,
        GBASE,
        XUGU,
        OCEANBASE,
        OCEANBASE_ORACLE,
        INFORMIX,
        ODPS,
        TERADATA,
        LOG4JDBC,
        PHOENIX,
        EDB,
        KYLIN,
        SQLITE,
        PRESTO,
        ELASTIC_SEARCH,
        CLICKHOUSE,
        KDB,
        POLARDB,
        POLARDBX,
        OSCAR
    );
    actual.push(format!(
        "SqlParserType\tSQL_PARSER_TYPE_DRUID\t{}",
        SqlParserType::SQL_PARSER_TYPE_DRUID
    ));
    actual.push(format!(
        "SqlParserType\tSQL_PARSER_TYPE_ANTLR\t{}",
        SqlParserType::SQL_PARSER_TYPE_ANTLR
    ));
    let mut original: Vec<String> = include_str!("fixtures/sqlparser_core/java-constants.tsv")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(original.len(), 88);
    assert_eq!(actual.len(), original.len());
    original.sort();
    actual.sort();
    assert_eq!(actual, original);
}
