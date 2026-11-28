package io.github.easy4j.seata.rust.jdbc;

import java.io.PrintWriter;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.sql.CallableStatement;
import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.SQLFeatureNotSupportedException;
import java.sql.SQLType;
import java.sql.Statement;
import java.sql.Types;
import java.util.Objects;
import java.util.logging.Logger;
import javax.sql.DataSource;

/**
 * 显式选择的 Connector/J 8.0.27 DATE/TIME 精确字符串数据源装饰器。
 * 原 JDBC 类型及 Seata Undo parser 不变，泛型 getObject 返回无损字符串，类型化字符串写入直接绑定给服务器。
 * 必须放在 Seata DataSourceProxy 内层；不是对已有数据源的自动升级或原字段类的隐式转换。
 */
public final class ExactTemporalDataSource implements DataSource {
    private final DataSource delegate;

    /** 参数为实际 Connector/J 数据源；保留认证、连接池与事务配置的既有拥有者。 */
    public ExactTemporalDataSource(DataSource delegate) { this.delegate = Objects.requireNonNull(delegate, "delegate"); }

    @Override
    public Connection getConnection() throws SQLException { return connection(delegate.getConnection()); }

    @Override
    public Connection getConnection(String user, String password) throws SQLException { return connection(delegate.getConnection(user,password)); }

    private static Connection connection(Connection target) throws SQLException {
        try {
            String version = target.getMetaData().getDriverVersion();
            if (Objects.isNull(version) || !version.startsWith("mysql-connector-java-8.0.27 ")) throw new SQLFeatureNotSupportedException("Exact temporal JDBC profile requires Connector/J 8.0.27");
            return (Connection) Proxy.newProxyInstance(Connection.class.getClassLoader(),new Class<?>[]{Connection.class},(proxy,method,args)->{
                Object special = wrapper(proxy,method,args);
                if (special != DELEGATE) return special;
                Object value = invoke(target,method,args);
                if (value instanceof Statement) return statement((Statement) value,(Connection) proxy);
                if (value instanceof DatabaseMetaData) return metadata((DatabaseMetaData) value,(Connection) proxy);
                return value;
            });
        } catch (SQLException | RuntimeException failure) {
            try { target.close(); } catch (SQLException closeFailure) { failure.addSuppressed(closeFailure); }
            throw failure;
        }
    }

    private static Object statement(Statement target, Connection owner) {
        Class<?> kind = target instanceof CallableStatement ? CallableStatement.class : target instanceof PreparedStatement ? PreparedStatement.class : Statement.class;
        return Proxy.newProxyInstance(kind.getClassLoader(),new Class<?>[]{kind},(proxy,method,args)->{
            Object special = wrapper(proxy,method,args);
            if (special != DELEGATE) return special;
            if ("getConnection".equals(method.getName())) return owner;
            if (target instanceof PreparedStatement && "setObject".equals(method.getName()) && Objects.nonNull(args) && args.length >= 3 && args[1] instanceof String) {
                Integer jdbc = args[2] instanceof Integer ? (Integer) args[2] : args[2] instanceof SQLType ? ((SQLType) args[2]).getVendorTypeNumber() : null;
                if (Objects.nonNull(jdbc) && (jdbc == Types.DATE || jdbc == Types.TIME)) {
                    ((PreparedStatement) target).setString((Integer) args[0],(String) args[1]);
                    return null;
                }
            }
            Object value = invoke(target,method,args);
            return value instanceof ResultSet ? result((ResultSet) value,(Statement) proxy) : value;
        });
    }

    private static DatabaseMetaData metadata(DatabaseMetaData target, Connection owner) {
        return (DatabaseMetaData) Proxy.newProxyInstance(DatabaseMetaData.class.getClassLoader(),new Class<?>[]{DatabaseMetaData.class},(proxy,method,args)->{
            Object special = wrapper(proxy,method,args);
            if (special != DELEGATE) return special;
            if ("getConnection".equals(method.getName())) return owner;
            Object value = invoke(target,method,args);
            return value instanceof ResultSet ? result((ResultSet) value,null) : value;
        });
    }

    private static ResultSet result(ResultSet target, Statement owner) {
        return (ResultSet) Proxy.newProxyInstance(ResultSet.class.getClassLoader(),new Class<?>[]{ResultSet.class},(proxy,method,args)->{
            Object special = wrapper(proxy,method,args);
            if (special != DELEGATE) return special;
            if ("getStatement".equals(method.getName())) return owner;
            if ("getObject".equals(method.getName()) && Objects.nonNull(args) && args.length == 1) {
                int index = args[0] instanceof Integer ? (Integer) args[0] : target.findColumn((String) args[0]);
                return ExactTemporalValueReader.read(target,index);
            }
            return invoke(target,method,args);
        });
    }

    private static final Object DELEGATE = new Object();

    private static Object wrapper(Object proxy, Method method, Object[] args) {
        String name = method.getName();
        if (method.getDeclaringClass() == Object.class) {
            if ("toString".equals(name)) return "ExactTemporalJdbcWrapper";
            if ("hashCode".equals(name)) return System.identityHashCode(proxy);
            if ("equals".equals(name)) return proxy == args[0];
        }
        if (("unwrap".equals(name) || "isWrapperFor".equals(name)) && Objects.nonNull(args) && args[0] instanceof Class<?> && ((Class<?>) args[0]).isInstance(proxy)) return "unwrap".equals(name) ? proxy : Boolean.TRUE;
        return DELEGATE;
    }

    private static Object invoke(Object target, Method method, Object[] args) throws Throwable {
        try { return method.invoke(target,args); }
        catch (InvocationTargetException failure) { throw failure.getCause(); }
    }

    @Override
    public PrintWriter getLogWriter() throws SQLException { return delegate.getLogWriter(); }
    @Override
    public void setLogWriter(PrintWriter writer) throws SQLException { delegate.setLogWriter(writer); }
    @Override
    public void setLoginTimeout(int seconds) throws SQLException { delegate.setLoginTimeout(seconds); }
    @Override
    public int getLoginTimeout() throws SQLException { return delegate.getLoginTimeout(); }
    @Override
    public Logger getParentLogger() throws SQLFeatureNotSupportedException { return delegate.getParentLogger(); }
    @Override
    public boolean isWrapperFor(Class<?> kind) throws SQLException { return Objects.nonNull(kind) && kind.isInstance(this) || delegate.isWrapperFor(kind); }
    @Override
    public <T> T unwrap(Class<T> kind) throws SQLException { return Objects.nonNull(kind) && kind.isInstance(this) ? kind.cast(this) : delegate.unwrap(kind); }
}
