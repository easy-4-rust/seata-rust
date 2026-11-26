package org.apache.seata.rm.datasource.util;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Locale;
import org.apache.seata.common.loader.EnhancedServiceLoader;
import org.apache.seata.common.loader.EnhancedServiceNotFoundException;
import org.apache.seata.common.loader.LoadLevel;
import org.apache.seata.sqlparser.SqlParserType;
import org.apache.seata.sqlparser.util.DbTypeParser;

/** 观察固定版原 Java 解析器发现、实例身份和 RM 委托。 */
public final class JdbcRmParserSpiOracle {
    @LoadLevel(name = SqlParserType.SQL_PARSER_TYPE_DRUID, order = 7)
    public static final class High implements DbTypeParser {
        @Override public String parseFromJdbcUrl(String url) { return "CHOSEN"; }
    }

    @LoadLevel(name = SqlParserType.SQL_PARSER_TYPE_DRUID, order = 3)
    public static final class Low implements DbTypeParser {
        @Override public String parseFromJdbcUrl(String url) { return "LOWER"; }
    }

    @LoadLevel(name = SqlParserType.SQL_PARSER_TYPE_ANTLR, order = 10)
    public static final class OtherName implements DbTypeParser {
        @Override public String parseFromJdbcUrl(String url) { return "OTHER"; }
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        List<String> values;
        if (args[0].equals("default")) {
            DbTypeParser first = JdbcUtils.getDbTypeParser();
            values = List.of(
                "testDbTypeParserLoading\t" + (first != null),
                "testDbTypeParserSingletonPattern\t" + (first == JdbcUtils.getDbTypeParser()),
                "provider\t" + first.getClass().getName(),
                "shared\t" + (first == EnhancedServiceLoader.load(DbTypeParser.class, SqlParserType.SQL_PARSER_TYPE_DRUID)),
                missingProvider()
            );
        } else if (args[0].equals("override")) {
            DbTypeParser parser = JdbcUtils.getDbTypeParser();
            values = List.of(
                "override\t" + (parser.getClass() == High.class),
                "rm-type\t" + JdbcUtils.getDbType("jdbc:h2:mem:test")
            );
        } else {
            throw new IllegalArgumentException(args[0]);
        }
        Files.write(Path.of(args[1]), values, StandardCharsets.UTF_8);
    }

    private static String missingProvider() {
        try {
            EnhancedServiceLoader.load(DbTypeParser.class, SqlParserType.SQL_PARSER_TYPE_ANTLR);
        } catch (EnhancedServiceNotFoundException error) {
            return "missing\t" + error.getClass().getName() + "\t" + error.getMessage();
        }
        throw new IllegalStateException("Unexpected ANTLR provider");
    }
}
