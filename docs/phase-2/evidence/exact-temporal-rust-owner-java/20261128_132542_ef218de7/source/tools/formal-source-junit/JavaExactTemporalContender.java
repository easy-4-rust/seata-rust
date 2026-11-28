import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.mysql.cj.jdbc.MysqlDataSource;
import io.github.easy4j.seata.rust.jdbc.ExactTemporalDataSource;
import io.seatarust.compat.mysql.JavaAtWriterAdmission;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.config.ConfigurationFactory;
import org.apache.seata.rm.RMClient;
import org.apache.seata.rm.datasource.ConnectionProxy;
import org.apache.seata.rm.datasource.DataSourceProxy;
import org.apache.seata.rm.datasource.exec.LockConflictException;
import org.apache.seata.rm.datasource.exec.LockWaitTimeoutException;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.apache.seata.tm.TMClient;
import org.apache.seata.tm.api.GlobalTransaction;
import org.apache.seata.tm.api.GlobalTransactionContext;
import org.springframework.mock.env.MockEnvironment;

/** 在原 Java 客户端中竞争 Rust 已提交的精确时间锁，并验证释放后的原阶段一及补偿。 */
public final class JavaExactTemporalContender {
    private static final ObjectMapper JSON = new ObjectMapper();

    /** 参数为控制器的自有目录，返回前保持原 RM 在线直到所有独立观察完成。 */
    public static void main(String[] args) throws Exception {
        Path work = Path.of(args[0]);
        JsonNode config = JSON.readTree(work.resolve("database.json").toFile());
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT,
                new MockEnvironment().withProperty("seata.config.type", "file")
                        .withProperty("seata.config.file.name", "file").withProperty("seata.registry.type", "file"));
        System.setProperty("service.vgroupMapping.cross_lock", "default");
        System.setProperty("service.default.grouplist", config.path("endpoint").asText());
        System.setProperty("transport.enableTmClientBatchSendRequest", "false");
        System.setProperty("transport.enableRmClientBatchSendRequest", "false");
        System.setProperty("client.rm.lock.retryTimes", "0");
        System.setProperty("client.undo.logSerialization", "jackson");
        System.setProperty("client.undo.compress.enable", "false");
        if (ConfigurationFactory.getInstance().getInt("client.rm.lock.retryTimes", -1) != 0)
            throw new IllegalStateException("Fail-fast retry policy differs");
        JacksonUndoLogParser.setZoneOffset(ZoneOffset.UTC);
        TMClient.init("reverse-temporal-java", "cross_lock"); RMClient.init("reverse-temporal-java", "cross_lock");
        int profile = config.path("profile").asInt();
        MysqlDataSource direct = new MysqlDataSource();
        direct.setURL(config.path("resource").asText()); direct.setUser("root");
        direct.setPassword(config.path("password").asText());
        direct.setUseSSL(false); direct.setAllowPublicKeyRetrieval(true); direct.setUseInformationSchema(true);
        direct.setTinyInt1isBit(profile == 5); direct.setServerTimezone("UTC");
        direct.setSessionVariables("time_zone='+00:00'");
        DataSourceProxy proxy = new DataSourceProxy(new ExactTemporalDataSource(direct));
        UUID server = UUID.fromString(config.path("server_uuid").asText());
        String schema = config.path("schema").asText();
        List<Object> rejected = new ArrayList<>();
        for (String name : new String[]{"date", "time"}) {
            String table = "date".equals(name) ? "temporal_dates" : "temporal_times";
            String key = "date".equals(name) ? "1582-10-05" : "-25:01:02.123456";
            GlobalTransaction tx = GlobalTransactionContext.getCurrentOrCreate();
            try (ConnectionProxy connection = proxy.getConnection()) {
                connection.setAutoCommit(false);
                JavaAtWriterAdmission.verify(connection, server, schema, profile);
                tx.begin(120000, "reverse-rejected-" + name);
                int before = value(connection.getTargetConnection(), table, key);
                update(connection, table, key);
                try {
                    connection.commit(); throw new IllegalStateException("Java bypassed the Rust lock");
                } catch (LockConflictException expected) {
                    rejected.add(Map.of("case", name, "xid", tx.getXid(), "lock", table + ":" + key,
                            "exception", expected.getCode().name()));
                } catch (LockWaitTimeoutException timeout) {
                    if (!(timeout.getCause() instanceof LockConflictException expected)) throw timeout;
                    rejected.add(Map.of("case", name, "xid", tx.getXid(), "lock", table + ":" + key,
                            "exception", expected.getCode().name()));
                }
                if (value(connection.getTargetConnection(), table, key) != before)
                    throw new IllegalStateException("Rejected original Java commit retained local changes");
                connection.rollback(); tx.rollback();
            }
        }
        write(work.resolve("rejected.json"), rejected);
        await(work.resolve("owner-released"));
        for (String name : new String[]{"date", "time"}) {
            String table = "date".equals(name) ? "temporal_dates" : "temporal_times";
            String key = "date".equals(name) ? "1582-10-05" : "-25:01:02.123456";
            GlobalTransaction tx = GlobalTransactionContext.getCurrentOrCreate();
            try (ConnectionProxy connection = proxy.getConnection()) {
                connection.setAutoCommit(false);
                JavaAtWriterAdmission.verify(connection, server, schema, profile);
                tx.begin(120000, "reverse-admitted-" + name);
                update(connection, table, key); connection.commit();
            }
            Map<String, Object> phase = new LinkedHashMap<>();
            phase.put("case", name); phase.put("xid", tx.getXid()); phase.put("resource", proxy.getResourceId());
            phase.put("lock", table + ":" + key); phase.put("profile", profile);
            try (Connection raw = direct.getConnection(); PreparedStatement query = raw.prepareStatement(
                    "SELECT branch_id,HEX(rollback_info),context FROM undo_log WHERE xid=?")) {
                query.setString(1, tx.getXid());
                try (ResultSet rows = query.executeQuery()) {
                    if (!rows.next()) throw new IllegalStateException("Java original Undo absent");
                    phase.put("branch_id", rows.getLong(1)); phase.put("undo_hex", rows.getString(2));
                    phase.put("context", rows.getString(3));
                    if (rows.next()) throw new IllegalStateException("Duplicate Java Undo");
                }
            }
            write(work.resolve("admitted-" + name + ".json"), phase);
            await(work.resolve("admitted-" + name + ".release"));
            tx.rollback();
            Files.writeString(work.resolve("finished-" + name), tx.getXid(), StandardOpenOption.CREATE_NEW);
            await(work.resolve("finished-" + name + ".ack"));
        }
        Files.writeString(work.resolve("finished"), "done", StandardOpenOption.CREATE_NEW);
        await(work.resolve("stop")); System.exit(0);
    }

    private static void update(Connection connection, String table, String key) throws SQLException {
        try (PreparedStatement statement = connection.prepareStatement("UPDATE " + table + " SET v=v+1 WHERE k=?")) {
            statement.setString(1, key);
            if (statement.executeUpdate() != 1) throw new IllegalStateException("Java DML count differs");
        }
    }
    private static int value(Connection connection, String table, String key) throws SQLException {
        try (PreparedStatement statement = connection.prepareStatement("SELECT v FROM " + table + " WHERE k=?")) {
            statement.setString(1, key);
            try (ResultSet rows = statement.executeQuery()) {
                if (!rows.next()) throw new IllegalStateException("Java business row missing");
                return rows.getInt(1);
            }
        }
    }
    private static void write(Path target, Object value) throws Exception {
        Files.write(target, JSON.writeValueAsBytes(value), StandardOpenOption.CREATE_NEW);
    }
    private static void await(Path signal) throws Exception {
        long deadline = System.nanoTime() + 120_000_000_000L;
        while (!Files.exists(signal)) {
            if (System.nanoTime() > deadline) throw new IllegalStateException("Missing signal " + signal);
            Thread.sleep(20);
        }
    }
}
