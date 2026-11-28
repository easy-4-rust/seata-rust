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
import java.sql.Statement;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import javax.sql.DataSource;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.config.ConfigurationFactory;
import org.apache.seata.rm.RMClient;
import org.apache.seata.rm.datasource.ConnectionProxy;
import org.apache.seata.rm.datasource.DataSourceProxy;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.apache.seata.tm.TMClient;
import org.apache.seata.tm.api.GlobalTransaction;
import org.apache.seata.tm.api.GlobalTransactionContext;
import org.springframework.mock.env.MockEnvironment;

/** 原 Java TC/RM 完整配置准入参与者；不替换原客户端类型、事务或补偿实现。 */
public final class JavaWriterJdbcProfileActor {
    private static final ObjectMapper JSON = new ObjectMapper();

    /** 参数为自有目录，输出原阶段一及控制信号；原 RM 保持在线直到控制器完成数据库观察。 */
    public static void main(String[] args) throws Exception {
        Path work = Path.of(args[0]);
        JsonNode config = JSON.readTree(work.resolve("database.json").toFile());
        int profile = config.path("profile").asInt();
        String endpoint = config.path("endpoint").asText();
        String resource = config.path("resource").asText();
        UUID server = UUID.fromString(config.path("server_uuid").asText());
        String schema = config.path("schema").asText();
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT,
                new MockEnvironment().withProperty("seata.config.type", "file")
                        .withProperty("seata.config.file.name", "file").withProperty("seata.registry.type", "file"));
        System.setProperty("service.vgroupMapping.cross_lock", "default");
        System.setProperty("service.default.grouplist", endpoint);
        System.setProperty("transport.enableTmClientBatchSendRequest", "false");
        System.setProperty("transport.enableRmClientBatchSendRequest", "false");
        System.setProperty("client.undo.logSerialization", "jackson");
        System.setProperty("client.undo.compress.enable", "false");
        if (!endpoint.equals(ConfigurationFactory.getInstance().getConfig("service.default.grouplist")))
            throw new IllegalStateException("TC endpoint differs");
        JacksonUndoLogParser.setZoneOffset(ZoneOffset.UTC);
        TMClient.init("jdbc-profile-java", "cross_lock");
        RMClient.init("jdbc-profile-java", "cross_lock");
        MysqlDataSource direct = new MysqlDataSource();
        direct.setURL(resource); direct.setUser("root");
        direct.setPassword(config.path("password").asText());
        direct.setUseSSL(false); direct.setAllowPublicKeyRetrieval(true);
        direct.setTinyInt1isBit(profile % 2 == 1); direct.setUseInformationSchema(true);
        direct.setServerTimezone("UTC"); direct.setSessionVariables("time_zone='+00:00'");
        DataSource actual = profile >= 5 ? new ExactTemporalDataSource(direct) : direct;
        DataSourceProxy proxy = new DataSourceProxy(actual);
        if (!resource.equals(proxy.getResourceId())) throw new IllegalStateException("Original resource ID differs");
        List<Object> rejects = new ArrayList<>();
        for (int bound = 1; bound <= 6; bound++) {
            if (bound == profile) continue;
            bind(direct, bound);
            try (ConnectionProxy connection = proxy.getConnection()) {
                connection.setAutoCommit(false);
                try {
                    JavaAtWriterAdmission.verify(connection, server, schema, profile);
                    throw new IllegalStateException("Mismatched binding admitted");
                } catch (SQLException expected) {
                    if (!"25000".equals(expected.getSQLState()) || !"jdbc-profile".equals(expected.getMessage())) throw expected;
                    connection.rollback();
                    rejects.add(Map.of("bound", bound, "actual", profile, "reason", expected.getMessage()));
                }
            }
        }
        write(work.resolve("rejections.json"), rejects);
        bind(direct, profile);
        GlobalTransaction transaction = GlobalTransactionContext.getCurrentOrCreate();
        try (ConnectionProxy connection = proxy.getConnection()) {
            connection.setAutoCommit(false);
            JavaAtWriterAdmission.verify(connection, server, schema, profile);
            Files.writeString(work.resolve("admission-held"), "verified", StandardOpenOption.CREATE_NEW);
            await(work.resolve("admission-release"));
            transaction.begin(180000, "java-jdbc-profile-" + profile);
            List<String> sql = new ArrayList<>();
            sql.add("UPDATE admission_business SET v=v+10 WHERE id=1");
            if (profile >= 5) {
                sql.add("UPDATE temporal_values SET d='1582-10-10',t='24:00:00.000001' WHERE id=1");
                sql.add("INSERT INTO temporal_values VALUES(3,'1000-01-01','-838:59:59')");
                sql.add("DELETE FROM temporal_values WHERE id=2");
                sql.add("UPDATE temporal_dates SET v=v+10 WHERE k='1582-10-05'");
                sql.add("UPDATE temporal_times SET v=v+10 WHERE k='-25:01:02.123456'");
                sql.add("INSERT INTO temporal_dates VALUES('1582-10-10',30)");
            }
            for (String command : sql) {
                try (PreparedStatement statement = connection.prepareStatement(command)) {
                    if (statement.executeUpdate() != 1) throw new IllegalStateException("DML count differs");
                }
            }
            connection.commit();
        }
        Map<String, Object> phase = new LinkedHashMap<>();
        phase.put("xid", transaction.getXid()); phase.put("profile", profile);
        phase.put("resource", proxy.getResourceId()); phase.put("process_id", ProcessHandle.current().pid());
        try (Connection raw = direct.getConnection(); PreparedStatement statement = raw.prepareStatement(
                "SELECT branch_id,HEX(rollback_info),context FROM undo_log WHERE xid=?")) {
            statement.setString(1, transaction.getXid());
            try (ResultSet rows = statement.executeQuery()) {
                if (!rows.next()) throw new IllegalStateException("Original Undo missing");
                phase.put("branch_id", rows.getLong(1)); phase.put("undo_hex", rows.getString(2));
                phase.put("undo_context", rows.getString(3));
                if (rows.next()) throw new IllegalStateException("Duplicate branch Undo");
            }
        }
        write(work.resolve("phase-one.json"), phase);
        await(work.resolve("decision-release"));
        if ("rollback".equals(config.path("direction").asText())) transaction.rollback();
        else transaction.commit();
        Files.writeString(work.resolve("finished"), transaction.getXid(), StandardOpenOption.CREATE_NEW);
        await(work.resolve("stop"));
        System.exit(0);
    }

    private static void bind(DataSource direct, int profile) throws SQLException {
        try (Connection raw = direct.getConnection(); PreparedStatement statement = raw.prepareStatement(
                "UPDATE seata_rs_at_resource_binding SET jdbc_profile=? WHERE slot=1")) {
            statement.setInt(1, profile);
            if (statement.executeUpdate() != 1) throw new IllegalStateException("Deployment binding missing");
        }
    }

    private static void write(Path target, Object value) throws Exception {
        Files.write(target, JSON.writeValueAsBytes(value), StandardOpenOption.CREATE_NEW);
    }

    private static void await(Path signal) throws Exception {
        long deadline = System.nanoTime() + 120_000_000_000L;
        while (!Files.exists(signal)) {
            if (System.nanoTime() > deadline) throw new IllegalStateException("Signal missing: " + signal);
            Thread.sleep(20);
        }
    }
}
