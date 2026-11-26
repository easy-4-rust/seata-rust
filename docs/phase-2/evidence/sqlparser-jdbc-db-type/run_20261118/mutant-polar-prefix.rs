use crate::{DbTypeParser, JavaString, MetadataError};

/// Druid 1.2.28 JDBC 前缀数据库类型识别。
/// 对应 Java: org.apache.seata.sqlparser.druid.DruidDbTypeParserImpl。
#[derive(Clone, Copy, Debug, Default)]
pub struct DruidDbTypeParserImpl;

// 首个匹配决定结果；OceanBase Oracle、Log4JDBC 和 PolarDB 的前缀存在重叠。
const PREFIXES: &[(&str, &str)] = &[
    ("jdbc:derby:", "derby"),
    ("jdbc:log4jdbc:derby:", "derby"),
    ("jdbc:mysql:", "mysql"),
    ("jdbc:cobar:", "mysql"),
    ("jdbc:log4jdbc:mysql:", "mysql"),
    ("jdbc:goldendb:", "mysql"),
    ("jdbc:mariadb:", "mariadb"),
    ("jdbc:tidb:", "tidb"),
    ("jdbc:oracle:", "oracle"),
    ("jdbc:log4jdbc:oracle:", "oracle"),
    ("jdbc:alibaba:oracle:", "ali_oracle"),
    ("jdbc:oceanbase:oracle:", "oceanbase_oracle"),
    ("jdbc:oceanbase:", "oceanbase"),
    ("jdbc:microsoft:", "sqlserver"),
    ("jdbc:log4jdbc:microsoft:", "sqlserver"),
    ("jdbc:sqlserver:", "sqlserver"),
    ("jdbc:log4jdbc:sqlserver:", "sqlserver"),
    ("jdbc:sybase:Tds:", "sybase"),
    ("jdbc:log4jdbc:sybase:", "sybase"),
    ("jdbc:jtds:", "jtds"),
    ("jdbc:log4jdbc:jtds:", "jtds"),
    ("jdbc:fake:", "mock"),
    ("jdbc:mock:", "mock"),
    ("jdbc:postgresql:", "postgresql"),
    ("jdbc:log4jdbc:postgresql:", "postgresql"),
    ("jdbc:edb:", "edb"),
    ("jdbc:hsqldb:", "hsql"),
    ("jdbc:log4jdbc:hsqldb:", "hsql"),
    ("jdbc:odps:", "odps"),
    ("jdbc:db2:", "db2"),
    ("jdbc:sqlite:", "sqlite"),
    ("jdbc:ingres:", "ingres"),
    ("jdbc:h2:", "h2"),
    ("jdbc:log4jdbc:h2:", "h2"),
    ("jdbc:lealone:", "lealone"),
    ("jdbc:mckoi:", "mock"),
    ("jdbc:cloudscape:", "cloudscape"),
    ("jdbc:informix-sqli:", "informix"),
    ("jdbc:log4jdbc:informix-sqli:", "informix"),
    ("jdbc:timesten:", "timesten"),
    ("jdbc:as400:", "as400"),
    ("jdbc:sapdb:", "sapdb"),
    ("jdbc:JSQLConnect:", "JSQLConnect"),
    ("jdbc:JTurbo:", "JTurbo"),
    ("jdbc:firebirdsql:", "firebirdsql"),
    ("jdbc:interbase:", "interbase"),
    ("jdbc:pointbase:", "pointbase"),
    ("jdbc:edbc:", "edbc"),
    ("jdbc:mimer:multi1:", "mimer"),
    ("jdbc:dm:", "dm"),
    ("jdbc:kingbase:", "kingbase"),
    ("jdbc:kingbase8:", "kingbase"),
    ("jdbc:gbase:", "gbase"),
    ("jdbc:xugu:", "xugu"),
    ("jdbc:log4jdbc:", "log4jdbc"),
    ("jdbc:hive:", "hive"),
    ("jdbc:hive2:", "hive"),
    ("jdbc:phoenix:", "phoenix"),
    ("jdbc:kylin:", "kylin"),
    ("jdbc:elastic:", "elastic_search"),
    ("jdbc:clickhouse:", "clickhouse"),
    ("jdbc:presto:", "presto"),
    ("jdbc:trino:", "trino"),
    ("jdbc:inspur:", "kdb"),
    ("jdbc:polardb", "polardb"),
    ("jdbc:polardbx:", "polardbx"),
    ("jdbc:polardb2:", "polardb2"),
    ("jdbc:highgo:", "highgo"),
    ("jdbc:pivotal:greenplum:", "greenplum"),
    ("jdbc:datadirect:greenplum:", "greenplum"),
    ("jdbc:opengauss:", "gaussdb"),
    ("jdbc:gaussdb:", "gaussdb"),
    ("jdbc:dws:iam:", "gaussdb"),
    ("jdbc:TAOS:", "taosdata"),
    ("jdbc:TAOS-RS:", "taosdata"),
    ("jdbc:oscar:", "oscar"),
    ("jdbc:sundb:", "sundb"),
];

impl DbTypeParser for DruidDbTypeParserImpl {
    /// 按区分大小写的 ASCII 前缀识别可空 jdbc_url。未识别返回 None。
    /// 对应 Java: DruidDbTypeParserImpl#parseFromJdbcUrl 调用 JdbcUtils.getDbType(url, null)。
    fn parse_from_jdbc_url(
        &self,
        jdbc_url: Option<&JavaString>,
    ) -> Result<Option<JavaString>, MetadataError> {
        let result = jdbc_url.and_then(|url| {
            let units = url.as_utf16();
            // 前缀只比较其自身码元，尾部的孤立代理码元也不会改变匹配结果。
            PREFIXES.iter().find_map(|(prefix, db_type)| {
                let input_prefix = units.get(..prefix.len())?;
                input_prefix
                    .iter()
                    .copied()
                    .eq(prefix.bytes().map(u16::from))
                    .then(|| JavaString::from(*db_type))
            })
        });
        Ok(result)
    }
}
