import com.mysql.cj.jdbc.MysqlDataSource;
import io.github.easy4j.seata.rust.jdbc.ExactTemporalDataSource;
import io.seatarust.compat.mysql.JavaAtWriterAdmission;
import java.sql.Connection;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import java.time.ZoneOffset;
import java.util.Objects;
import java.util.UUID;
import javax.sql.DataSource;
import org.apache.seata.rm.datasource.ConnectionProxy;
import org.apache.seata.rm.datasource.DataSourceProxy;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.mockito.Mockito;

/** 真实 JDBC 原事务准入探针；仅 Seata 数据源身份使用替身，不声称验证 TC/RM 回调。 */
public final class JavaWriterJdbcProfileProbe {
    private static int accepted;
    private static int rejected;

    public static void main(String[] args) throws Exception {
        JacksonUndoLogParser.setZoneOffset(ZoneOffset.UTC);
        MysqlDataSource admin = source(args[0], true);
        UUID server;
        String schema;
        try (Connection connection = admin.getConnection(); Statement sql = connection.createStatement()) {
            try (ResultSet row = sql.executeQuery("SELECT @@server_uuid,DATABASE()")) {
                row.next(); server = UUID.fromString(row.getString(1)); schema = row.getString(2);
            }
            sql.execute("CREATE TABLE admission_business(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB");
            sql.execute("INSERT INTO admission_business VALUES(1,0)");
        }
        for (int actual = 1; actual <= 6; actual++) {
            MysqlDataSource driver = source(args[0], actual % 2 == 1);
            DataSource data = actual >= 5 ? new ExactTemporalDataSource(driver) : driver;
            for (int bound = 1; bound <= 6; bound++) {
                binding(admin, args[0], actual % 2, bound);
                try (Connection raw = data.getConnection()) {
                    raw.setAutoCommit(false);
                    try (Statement sql = raw.createStatement()) { sql.execute("SET SESSION time_zone='+00:00'"); }
                    ConnectionProxy proxy = proxy(data, raw, args[0]);
                    check(proxy, server, schema, actual, actual == bound, null);
                    if (actual == bound) {
                        // 独立连接的部署变更必须真实超时，原事务回滚后才能执行。
                        try (Connection other = admin.getConnection(); Statement sql = other.createStatement()) {
                            sql.execute("SET SESSION innodb_lock_wait_timeout=1");
                            try {
                                sql.executeUpdate("UPDATE seata_rs_at_resource_binding SET jdbc_profile=NULL WHERE slot=1");
                                throw new AssertionError("binding lock missing");
                            } catch (SQLException expected) { if (expected.getErrorCode() != 1205) throw expected; }
                            raw.rollback();
                            if (sql.executeUpdate("UPDATE seata_rs_at_resource_binding SET jdbc_profile=NULL WHERE slot=1") != 1)
                                throw new AssertionError("binding not released");
                        }
                    }
                    raw.rollback();
                }
            }
            binding(admin, args[0], actual % 2, actual);
            try (Connection raw = data.getConnection()) {
                raw.setAutoCommit(false);
                try (Statement sql = raw.createStatement()) { sql.execute("SET SESSION time_zone='+00:00'"); }
                ConnectionProxy proxy = proxy(data, raw, args[0]);
                check(proxy, server, schema, actual == 6 ? 5 : actual + 1, false, null);
                check(proxy, server, schema, 0, false, "jdbc-profile");
                check(proxy, server, schema, 7, false, "jdbc-profile");
                if (actual >= 3) {
                    try (Statement sql = raw.createStatement()) { sql.execute("SET SESSION time_zone='+01:00'"); }
                    check(proxy, server, schema, actual, false, "utc-profile");
                }
                raw.rollback();
            }
        }
        binding(admin, args[0], 1, null);
        try (Connection raw = admin.getConnection()) {
            raw.setAutoCommit(false);
            ConnectionProxy proxy = proxy(admin, raw, args[0]);
            check(proxy, server, schema, 1, false, "jdbc-profile");
            JavaAtWriterAdmission.verify(proxy, server, schema);
            raw.rollback();
            try (Statement sql = raw.createStatement()) { sql.execute("ALTER TABLE seata_rs_at_resource_binding DROP CHECK ck_seata_rs_at_jdbc_profile, DROP COLUMN jdbc_profile"); }
            raw.commit();
            JavaAtWriterAdmission.verify(proxy, server, schema);
            try { JavaAtWriterAdmission.verify(proxy, server, schema, 1); throw new AssertionError("missing column admitted"); }
            catch (SQLException expected) { if (expected.getErrorCode() != 1054) throw expected; }
            raw.rollback();
        }
        try (Connection raw = admin.getConnection(); Statement sql = raw.createStatement();
             ResultSet row = sql.executeQuery("SELECT v FROM admission_business WHERE id=1")) {
            if (!row.next() || row.getInt(1) != 0) throw new AssertionError("rejection changed business");
        }
        System.out.println("JAVA_WRITER_JDBC_PROFILE_OK accepted=" + accepted + " rejected=" + rejected + " locks=6 legacy_null_and_missing=true");
        System.exit(0);
    }

    private static MysqlDataSource source(String url, boolean bool) throws SQLException {
        MysqlDataSource source = new MysqlDataSource();
        source.setURL(url); source.setUser("root");
        source.setPassword(Objects.requireNonNull(System.getenv("SEATA_MYSQL_PASSWORD")));
        source.setTinyInt1isBit(bool); source.setServerTimezone("UTC");
        source.setUseSSL(false); source.setAllowPublicKeyRetrieval(true);
        return source;
    }

    private static ConnectionProxy proxy(DataSource data, Connection raw, String resource) {
        DataSourceProxy identity = Mockito.mock(DataSourceProxy.class);
        Mockito.when(identity.getResourceId()).thenReturn(resource);
        Mockito.when(identity.getTargetDataSource()).thenReturn(data);
        return new ConnectionProxy(identity, raw);
    }

    private static void binding(DataSource source, String resource, int tiny, Integer profile) throws SQLException {
        try (Connection raw = source.getConnection(); java.sql.PreparedStatement sql = raw.prepareStatement(
                "REPLACE INTO seata_rs_at_resource_binding(slot,resource_id,tinyint1_is_bit,jdbc_profile) VALUES(1,?,?,?)")) {
            sql.setBytes(1, resource.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            sql.setInt(2, tiny); sql.setObject(3, profile); sql.executeUpdate();
        }
    }

    private static void check(ConnectionProxy proxy, UUID server, String schema, int profile,
                              boolean success, String reason) throws SQLException {
        try {
            JavaAtWriterAdmission.verify(proxy, server, schema, profile);
            if (!success) throw new AssertionError("incorrect profile admitted: " + profile);
            accepted++;
        } catch (SQLException error) {
            if (success || !"25000".equals(error.getSQLState())
                    || (Objects.nonNull(reason) && !reason.equals(error.getMessage()))) throw error;
            rejected++;
        }
    }
}
