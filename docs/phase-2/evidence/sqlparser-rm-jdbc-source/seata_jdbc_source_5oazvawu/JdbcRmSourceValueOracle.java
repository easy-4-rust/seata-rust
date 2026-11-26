import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.apache.seata.rm.datasource.util.JdbcUtils;

/** 用原实现观察 JdbcUtilsTest 中已有 Rust 对应项的返回值。 */
public final class JdbcRmSourceValueOracle {
    private static final String MYSQL = "jdbc:mysql://localhost:3306/test";

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        List<String> rows = new ArrayList<>();
        type(rows, "testGetDbTypeWithMySQLUrl", MYSQL);
        type(rows, "testGetDbTypeWithOracleUrl", "jdbc:oracle:thin:@localhost:1521:orcl");
        type(rows, "testGetDbTypeWithPostgreSQLUrl", "jdbc:postgresql://localhost:5432/test");
        type(rows, "testGetDbTypeWithH2Url", "jdbc:h2:mem:test");
        resource(rows, "testBuildResourceIdWithQueryString", MYSQL + "?useSSL=false&serverTimezone=UTC");
        resource(rows, "testBuildResourceIdWithoutQueryString", MYSQL);
        resource(rows, "testBuildResourceIdWithEmptyQueryString", MYSQL + "?");
        resource(rows, "testBuildResourceIdWithMultipleQuestionMarks", MYSQL + "?param1=value1?param2=value2");
        Files.write(Path.of(args[0]), rows, StandardCharsets.UTF_8);
    }

    private static void type(List<String> rows, String test, String url) {
        rows.add(test + "\t" + JdbcUtils.getDbType(url));
    }

    private static void resource(List<String> rows, String test, String url) {
        rows.add(test + "\t" + JdbcUtils.buildResourceId(url));
    }
}
