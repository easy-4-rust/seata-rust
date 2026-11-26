import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import javax.sql.DataSource;
import javax.sql.XAConnection;
import javax.sql.XADataSource;
import org.apache.seata.rm.BaseDataSourceResource;
import org.apache.seata.rm.DefaultResourceManager;
import org.apache.seata.rm.datasource.util.JdbcUtils;
import org.mockito.MockedStatic;
import org.mockito.Mockito;

/** 记录固定版原 Java 的 JDBC/XA 初始化及清理顺序。 */
public final class JdbcRmInitializationOracle {
    private record Case(
        String name, boolean xa, String url, String acquire, String metadata,
        boolean closeConnectionFails, boolean closeXaFails
    ) {}

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ENGLISH);
        String mysql = "jdbc:mysql://localhost:3306/test?param=1";
        String h2 = "jdbc:h2:mem:test?param=1";
        List<Case> cases = List.of(
            new Case("jdbc-ok", false, mysql, "", "", false, false),
            new Case("jdbc-acquire-sql", false, mysql, "sql", "", false, false),
            new Case("jdbc-null-metadata", false, mysql, "", "null", false, false),
            new Case("jdbc-null-url", false, null, "", "", false, false),
            new Case("jdbc-null-connection", false, mysql, "null", "", false, false),
            new Case("jdbc-unknown-driver", false, "jdbc:unknown:db?param=1", "", "", false, false),
            new Case("jdbc-close-sql", false, mysql, "", "", true, false),
            new Case("jdbc-metadata-and-close-sql", false, mysql, "", "sql", true, false),
            new Case("jdbc-null-metadata-and-close-sql", false, mysql, "", "null", true, false),
            new Case("xa-ok", true, h2, "", "", false, false),
            new Case("xa-acquire-sql", true, h2, "sql", "", false, false),
            new Case("xa-null-connection", true, h2, "null", "", false, false),
            new Case("xa-inner-acquire-sql", true, h2, "inner-sql", "", false, false),
            new Case("xa-close-sql", true, h2, "", "", false, true),
            new Case("xa-inner-close-sql", true, h2, "", "", true, false),
            new Case("xa-both-close-sql", true, h2, "", "", true, true),
            new Case("xa-close-replaces-runtime-error", true, h2, "", "null", true, true)
        );
        List<String> rows = new ArrayList<>();
        for (Case input : cases) {
            rows.add(observe(input));
        }
        Files.write(Path.of(args[0]), rows, StandardCharsets.UTF_8);
    }

    private static String observe(Case input) {
        List<String> steps = new ArrayList<>();
        DatabaseMetaData metadata = Mockito.mock(DatabaseMetaData.class, invocation -> {
            if (invocation.getMethod().getName().equals("getURL")) {
                steps.add("url");
                return input.url();
            }
            return Mockito.RETURNS_DEFAULTS.answer(invocation);
        });
        Connection connection = Mockito.mock(Connection.class, invocation -> {
            switch (invocation.getMethod().getName()) {
                case "getMetaData":
                    steps.add("metadata");
                    if (input.metadata().equals("sql")) {
                        throw new SQLException("metadata");
                    }
                    return input.metadata().equals("null") ? null : metadata;
                case "close":
                    steps.add("close-connection");
                    if (input.closeConnectionFails()) {
                        throw new SQLException("close-connection");
                    }
                    return null;
                default:
                    return Mockito.RETURNS_DEFAULTS.answer(invocation);
            }
        });
        XAConnection xa = Mockito.mock(XAConnection.class, invocation -> {
            switch (invocation.getMethod().getName()) {
                case "getConnection":
                    steps.add("get-connection");
                    if (input.acquire().equals("inner-sql")) {
                        throw new SQLException("get-connection");
                    }
                    return connection;
                case "close":
                    steps.add("close-xa");
                    if (input.closeXaFails()) {
                        throw new SQLException("close-xa");
                    }
                    return null;
                default:
                    return Mockito.RETURNS_DEFAULTS.answer(invocation);
            }
        });
        DataSource jdbc = Mockito.mock(DataSource.class, invocation -> {
            return switch (invocation.getMethod().getName()) {
                case "getConnection" -> acquire(steps, "get-connection", input.acquire(), connection);
                case "toString" -> "jdbc-probe";
                default -> Mockito.RETURNS_DEFAULTS.answer(invocation);
            };
        });
        XADataSource xaSource = Mockito.mock(XADataSource.class, invocation -> {
            return switch (invocation.getMethod().getName()) {
                case "getXAConnection" -> acquire(steps, "get-xa", input.acquire(), xa);
                case "toString" -> "xa-probe";
                default -> Mockito.RETURNS_DEFAULTS.answer(invocation);
            };
        });
        BaseDataSourceResource<?> resource = Mockito.mock(BaseDataSourceResource.class, invocation -> {
            switch (invocation.getMethod().getName()) {
                case "setResourceGroupId" -> steps.add("group=" + invocation.getArgument(0));
                case "setResourceId" -> steps.add("id=" + invocation.getArgument(0));
                case "setDriver" -> steps.add("driver=" + invocation.getArgument(0).getClass().getName());
                case "setDbType" -> steps.add("db=" + invocation.getArgument(0));
                default -> { return Mockito.RETURNS_DEFAULTS.answer(invocation); }
            }
            return null;
        });
        DefaultResourceManager manager = Mockito.mock(DefaultResourceManager.class);
        Mockito.doAnswer(invocation -> {
            steps.add(invocation.getArgument(0) == resource ? "register" : "register-other");
            return null;
        }).when(manager).registerResource(Mockito.any());
        String result;
        try (MockedStatic<DefaultResourceManager> scope = Mockito.mockStatic(DefaultResourceManager.class)) {
            scope.when(DefaultResourceManager::get).thenReturn(manager);
            try {
                if (input.xa()) {
                    JdbcUtils.initXADataSourceResource(resource, xaSource, "group");
                } else {
                    JdbcUtils.initDataSourceResource(resource, jdbc, "group");
                }
                result = "ok";
            } catch (RuntimeException error) {
                result = describe(error);
            }
        }
        return input.name() + "\t" + String.join("|", steps) + "\t" + result;
    }

    private static <T> T acquire(List<String> steps, String name, String failure, T result)
        throws SQLException {
        steps.add(name);
        if (failure.equals("sql")) {
            throw new SQLException(name);
        }
        return failure.equals("null") ? null : result;
    }

    private static String describe(Throwable error) {
        String text = error.getClass().getName() + ":" + error.getMessage();
        if (error.getCause() != null) {
            text += " cause(" + describe(error.getCause()) + ")";
        }
        for (Throwable suppressed : error.getSuppressed()) {
            text += " suppressed(" + describe(suppressed) + ")";
        }
        return text;
    }
}
