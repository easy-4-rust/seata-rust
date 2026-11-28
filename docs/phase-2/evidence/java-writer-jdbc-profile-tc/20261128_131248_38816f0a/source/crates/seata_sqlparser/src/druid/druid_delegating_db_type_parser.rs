use crate::{
    DbTypeParser, DbTypeParserRegistration, DruidDbTypeParserImpl, JavaString, MetadataError,
    SqlParserType,
};

/// 以 Druid 服务名注册并委托给固定版本的原生 JDBC 类型解析器。
/// 对应 Java: org.apache.seata.sqlparser.druid.DruidDelegatingDbTypeParser。
pub struct DruidDelegatingDbTypeParser {
    implementation: &'static DruidDbTypeParserImpl,
}

static IMPLEMENTATION: DruidDbTypeParserImpl = DruidDbTypeParserImpl;
static PROVIDER: DruidDelegatingDbTypeParser = DruidDelegatingDbTypeParser {
    implementation: &IMPLEMENTATION,
};

inventory::submit! {
    DbTypeParserRegistration::new(SqlParserType::SQL_PARSER_TYPE_DRUID, 0, &PROVIDER)
}

impl DbTypeParser for DruidDelegatingDbTypeParser {
    /// 将可空 jdbc_url 委托给已绑定的实现，原样返回可空结果或错误。
    /// 对应 Java: DruidDelegatingDbTypeParser#parseFromJdbcUrl。
    fn parse_from_jdbc_url(
        &self,
        jdbc_url: Option<&JavaString>,
    ) -> Result<Option<JavaString>, MetadataError> {
        self.implementation.parse_from_jdbc_url(jdbc_url)
    }
}
