import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;
import javax.sql.DataSource;
import javax.sql.XAConnection;
import javax.sql.XADataSource;
import org.apache.seata.rm.BaseDataSourceResource;
import org.apache.seata.rm.DefaultResourceManager;
import org.apache.seata.rm.datasource.util.JdbcUtils;
import org.mockito.MockedStatic;
import org.mockito.Mockito;

/** 记录原 JdbcUtilsTest 七个初始化输入在原生产入口的字段、关闭和登记。 */
public final class JdbcInitCaseOracle {
    private static final String MYSQL = "jdbc:mysql://localhost:3306/test?useSSL=false&serverTimezone=UTC";
    private static final String H2 = "jdbc:h2:mem:test";

    private enum Input { SUCCESS, ACQUIRE_SQL, NULL, CLOSE_SQL }

    private JdbcInitCaseOracle() {
    }

    /** 运行原普通 JDBC 和 XA 七个场景，输出真实轨迹与异常链。 */
    public static void main(String[] args) {
        observe("testInitDataSourceResourceSuccess", false, Input.SUCCESS);
        observe("testInitDataSourceResourceWithSQLException", false, Input.ACQUIRE_SQL);
        observe("testInitDataSourceResourceWithNullMetaData", false, Input.NULL);
        observe("testInitXADataSourceResourceSuccess", true, Input.SUCCESS);
        observe("testInitXADataSourceResourceWithSQLException", true, Input.ACQUIRE_SQL);
        observe("testInitXADataSourceResourceWithNullXAConnection", true, Input.NULL);
        observe("testInitXADataSourceResourceEnsuresConnectionClosed", true, Input.CLOSE_SQL);
    }

    private static void observe(String method, boolean xa, Input input) {
        List<String> trace = new ArrayList<>();
        DatabaseMetaData metadata = Mockito.mock(DatabaseMetaData.class, invocation -> {
            if ("getURL".equals(invocation.getMethod().getName())) {
                trace.add("url");
                return xa ? H2 : MYSQL;
            }
            return Mockito.RETURNS_DEFAULTS.answer(invocation);
        });
        Connection connection = Mockito.mock(Connection.class, invocation -> {
            return switch (invocation.getMethod().getName()) {
                case "getMetaData" -> {
                    trace.add("metadata");
                    yield !xa && input == Input.NULL ? null : metadata;
                }
                case "close" -> {
                    trace.add("close-connection");
                    yield null;
                }
                default -> Mockito.RETURNS_DEFAULTS.answer(invocation);
            };
        });
        XAConnection outer = Mockito.mock(XAConnection.class, invocation -> {
            return switch (invocation.getMethod().getName()) {
                case "getConnection" -> {
                    trace.add("get-connection");
                    yield connection;
                }
                case "close" -> {
                    trace.add("close-xa");
                    if (input == Input.CLOSE_SQL) {
                        throw new SQLException("Close failed");
                    }
                    yield null;
                }
                default -> Mockito.RETURNS_DEFAULTS.answer(invocation);
            };
        });
        DataSource normalSource = Mockito.mock(DataSource.class, invocation -> {
            return switch (invocation.getMethod().getName()) {
                case "getConnection" -> {
                    trace.add("get-connection");
                    if (input == Input.ACQUIRE_SQL) {
                        throw new SQLException("Connection failed");
                    }
                    yield connection;
                }
                case "toString" -> "jdbc-source";
                default -> Mockito.RETURNS_DEFAULTS.answer(invocation);
            };
        });
        XADataSource xaSource = Mockito.mock(XADataSource.class, invocation -> {
            return switch (invocation.getMethod().getName()) {
                case "getXAConnection" -> {
                    trace.add("get-xa");
                    if (input == Input.ACQUIRE_SQL) {
                        throw new SQLException("XA connection failed");
                    }
                    yield input == Input.NULL ? null : outer;
                }
                case "toString" -> "xa-source";
                default -> Mockito.RETURNS_DEFAULTS.answer(invocation);
            };
        });
        BaseDataSourceResource<?> resource = Mockito.mock(BaseDataSourceResource.class, invocation -> {
            switch (invocation.getMethod().getName()) {
                case "setResourceGroupId" -> trace.add("group=" + invocation.getArgument(0));
                case "setResourceId" -> trace.add("id=" + invocation.getArgument(0));
                case "setDriver" -> trace.add("driver=" + invocation.getArgument(0).getClass().getName());
                case "setDbType" -> trace.add("db=" + invocation.getArgument(0));
                default -> {
                    return Mockito.RETURNS_DEFAULTS.answer(invocation);
                }
            }
            return null;
        });
        DefaultResourceManager manager = Mockito.mock(DefaultResourceManager.class);
        Mockito.doAnswer(invocation -> {
            trace.add(invocation.getArgument(0) == resource ? "register" : "register-other");
            return null;
        }).when(manager).registerResource(Mockito.any());
        String result = "ok";
        try (MockedStatic<DefaultResourceManager> scope = Mockito.mockStatic(DefaultResourceManager.class)) {
            scope.when(DefaultResourceManager::get).thenReturn(manager);
            try {
                if (xa) {
                    JdbcUtils.initXADataSourceResource(resource, xaSource, "xa-group");
                } else {
                    JdbcUtils.initDataSourceResource(resource, normalSource, "test-group");
                }
            } catch (RuntimeException error) {
                result = describe(error);
            }
        }
        System.out.println("INIT\t" + method + "\t" + String.join("|", trace) + "\t" + result);
    }

    private static String describe(Throwable error) {
        String value = error.getClass().getName() + ":" + error.getMessage();
        Throwable cause = error.getCause();
        if (java.util.Objects.nonNull(cause)) {
            value += " cause(" + describe(cause) + ")";
        }
        return value;
    }
}
