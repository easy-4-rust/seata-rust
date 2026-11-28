package org.apache.seata.rm.datasource.sql.struct.cache;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.charset.StandardCharsets;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.sql.Types;
import io.github.easy4j.seata.rust.jdbc.ExactTemporalValueReader;
import io.github.easy4j.seata.rust.jdbc.ExactTemporalDataSource;
import com.mysql.cj.jdbc.MysqlDataSource;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.ArrayList;
import java.util.Date;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Properties;
import java.util.TimeZone;
import org.apache.seata.rm.datasource.sql.struct.Field;
import org.apache.seata.rm.datasource.sql.struct.Row;
import org.apache.seata.rm.datasource.sql.struct.TableRecords;
import org.apache.seata.rm.datasource.undo.BranchUndoLog;
import org.apache.seata.rm.datasource.undo.SQLUndoLog;
import org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser;
import org.apache.seata.sqlparser.SQLType;
import org.apache.seata.sqlparser.struct.TableMeta;

/** 真实 MySQL 时间类型的原驱动、原 Undo 与 JDBC 重绑定诊断；异常也是结果，不能计为兼容通过。 */
public final class ExactTemporalColumnOracle extends MysqlTableMetaCache {
    private static final String[] TABLES = {"probe_date", "probe_time", "probe_timestamp", "probe_time_0", "probe_time_1", "probe_time_2", "probe_time_3", "probe_time_4", "probe_time_5"};

    public static void main(String[] args) throws Exception {
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        String sessionZone = args[1];
        String adapterMode = args[2];
        String serverPrepare = args[3];
        String driverZone = "+00:00".equals(sessionZone) ? "UTC" : "Asia/Shanghai";
        Class.forName("com.mysql.cj.jdbc.Driver");
        Properties properties = new Properties();
        properties.setProperty("user", "root");
        properties.setProperty("password", Objects.requireNonNull(System.getenv("SEATA_MYSQL_PASSWORD")));
        properties.setProperty("useSSL", "false");
        properties.setProperty("allowPublicKeyRetrieval", "true");
        properties.setProperty("useInformationSchema", "true");
        properties.setProperty("serverTimezone", driverZone);
        properties.setProperty("useServerPrepStmts", serverPrepare);
        properties.setProperty("useCursorFetch", "false");
        String url = "jdbc:mysql://127.0.0.1:" + System.getenv("SEATA_MYSQL_PORT") + "/" + System.getenv("SEATA_MYSQL_DATABASE");
        JacksonUndoLogParser.setZoneOffset(java.time.ZoneOffset.UTC);
        JacksonUndoLogParser parser = new JacksonUndoLogParser();
        parser.init();
        Map<String, Object> report = new LinkedHashMap<>();
        report.put("jvm_zone", TimeZone.getDefault().getID());
        report.put("session_zone", sessionZone);
        report.put("driver_zone", driverZone);
        report.put("undo_zone", "UTC");
        report.put("adapter_mode", adapterMode);
        report.put("server_prepare", Boolean.parseBoolean(serverPrepare));
        List<Object> cases = new ArrayList<>();
        report.put("cases", cases);
        MysqlDataSource actualSource = new MysqlDataSource();
        actualSource.setURL(url + "?useSSL=false&allowPublicKeyRetrieval=true&useInformationSchema=true&serverTimezone=" + driverZone + "&useServerPrepStmts=" + serverPrepare + "&useCursorFetch=false");
        actualSource.setUser("root"); actualSource.setPassword(properties.getProperty("password"));
        try (Connection connection = "datasource".equals(adapterMode) ? new ExactTemporalDataSource(actualSource).getConnection() : DriverManager.getConnection(url, properties)) {
            report.put("driver", connection.getMetaData().getDriverVersion());
            report.put("server", connection.getMetaData().getDatabaseProductVersion());
            try (Statement statement = connection.createStatement()) {
                statement.execute("SET time_zone='" + sessionZone + "'");
                statement.execute("SET sql_mode='STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION'");
            }
            ExactTemporalColumnOracle cache = new ExactTemporalColumnOracle();
            for (String table : TABLES) {
                TableMeta metadata = cache.fetchSchema(connection, table);
                List<Integer> ids = new ArrayList<>();
                try (Statement statement = connection.createStatement(); ResultSet rows = statement.executeQuery("SELECT id FROM " + table + " ORDER BY id")) {
                    while (rows.next()) ids.add(rows.getInt(1));
                }
                for (int id : ids) {
                    Map<String, Object> item = new LinkedHashMap<>();
                    cases.add(item);
                    item.put("table", table);
                    item.put("id", id);
                    item.put("jdbc_type", metadata.getAllColumns().get("v").getDataType());
                    try (Statement statement = connection.createStatement(); ResultSet rows = statement.executeQuery("SELECT CAST(v AS CHAR) FROM " + table + " WHERE id=" + id)) {
                        if (!rows.next()) throw new AssertionError("missing physical row");
                        item.put("physical_before", rows.getString(1));
                    }
                    String stage = "read_original";
                    try {
                        TableRecords records;
                        try (PreparedStatement statement = connection.prepareStatement("SELECT * FROM " + table + " WHERE id=?")) {
                            statement.setInt(1, id);
                            item.put("statement_class", statement.getClass().getName());
                            if ("datasource".equals(adapterMode) && statement.getConnection() != connection) throw new AssertionError("statement connection escaped adapter");
                            try (ResultSet rows = statement.executeQuery()) {
                                if ("datasource".equals(adapterMode) && rows.getStatement() != statement) throw new AssertionError("result statement escaped adapter");
                                com.mysql.cj.protocol.ResultsetRows container = rows.unwrap(com.mysql.cj.jdbc.result.ResultSetInternalMethods.class).getRows();
                                item.put("row_binary", ((com.mysql.cj.protocol.ResultsetRow) container.get(0)).isBinaryEncoded());
                                records = TableRecords.buildRecords(metadata, ("default".equals(adapterMode) || "datasource".equals(adapterMode)) ? rows : exactRows(rows, adapterMode));
                            }
                        }
                        Object original = value(records);
                        item.put("original", describe(original));
                        SQLUndoLog log = new SQLUndoLog();
                        log.setSqlType(SQLType.UPDATE);
                        log.setTableName(table);
                        log.setBeforeImage(records);
                        log.setAfterImage(records);
                        BranchUndoLog branch = new BranchUndoLog();
                        branch.setXid("127.0.0.1:8091:42");
                        branch.setBranchId(Long.MAX_VALUE);
                        branch.setSqlUndoLogs(new ArrayList<>(List.of(log)));
                        stage = "encode_original";
                        byte[] encoded = parser.encode(branch);
                        Files.write(out.resolve(table + "-" + id + ".json"), encoded);
                        item.put("undo_utf8", new String(encoded, StandardCharsets.UTF_8));
                        stage = "decode_original";
                        BranchUndoLog decoded = parser.decode(encoded);
                        if (decoded.getBranchId() != Long.MAX_VALUE || !Objects.equals(decoded.getXid(), branch.getXid())) throw new AssertionError("branch identity changed");
                        Object recovered = value(decoded.getSqlUndoLogs().get(0).getBeforeImage());
                        item.put("decoded", describe(recovered));
                        item.put("same_class", Objects.isNull(original) ? Objects.isNull(recovered) : Objects.nonNull(recovered) && original.getClass() == recovered.getClass());
                        item.put("same_value", Objects.deepEquals(original, recovered));
                        stage = "rebind_original";
                        try (PreparedStatement statement = connection.prepareStatement("INSERT INTO " + table + "_restored(id,v) VALUES(?,?) ON DUPLICATE KEY UPDATE v=VALUES(v)")) {
                            statement.setInt(1, id);
                            int jdbcType = metadata.getAllColumns().get("v").getDataType();
                            if (("read_write_text".equals(adapterMode) || "read_bytes_write_text".equals(adapterMode)) && recovered instanceof String && (jdbcType == Types.DATE || jdbcType == Types.TIME)) {
                                statement.setString(2, (String) recovered);
                            } else {
                                statement.setObject(2, recovered, jdbcType);
                            }
                            statement.executeUpdate();
                        }
                        try (Statement statement = connection.createStatement(); ResultSet rows = statement.executeQuery("SELECT CAST(v AS CHAR) FROM " + table + "_restored WHERE id=" + id)) {
                            if (!rows.next()) throw new AssertionError("missing rebound row");
                            String actual = rows.getString(1);
                            item.put("physical_rebound", actual);
                            item.put("same_physical", Objects.equals(item.get("physical_before"), actual));
                        }
                        item.put("status", "OBSERVED");
                    } catch (Exception failure) {
                        item.put("status", "ORIGINAL_EXCEPTION");
                        item.put("failure_stage", stage);
                        item.put("exception", failure.getClass().getName());
                        item.put("message", failure.getMessage());
                    }
                }
            }
            if ("datasource".equals(adapterMode)) {
                componentChecks(connection);
                report.put("adapter_lifecycle_verified",true);
                report.put("sqltype_binding_verified",true);
                report.put("null_semantics_verified",true);
                report.put("rollback_verified",true);
                report.put("exception_state_preserved",true);
                report.put("connection_chain_preserved",true);
            }
        }
        Files.write(out.resolve("observations.json"), new ObjectMapper().writerWithDefaultPrettyPrinter().writeValueAsBytes(report));
        System.out.println("temporal_original_cases=" + cases.size());
    }

    private static void componentChecks(Connection connection) throws Exception {
        if (!connection.isWrapperFor(Connection.class) || connection.unwrap(Connection.class) != connection || connection.getMetaData().getConnection() != connection) throw new AssertionError("connection wrapper chain");
        boolean autoCommit = connection.getAutoCommit();
        connection.setAutoCommit(false);
        try {
            try (PreparedStatement statement=connection.prepareStatement("INSERT INTO probe_adapter_write_probe VALUES(1,?,?)")) {
                statement.setObject(1,"-00:00:00.000001",java.sql.JDBCType.TIME);
                statement.setObject(2,"1582-10-05",java.sql.JDBCType.DATE);
                if(statement.executeUpdate()!=1)throw new AssertionError("SQLType insert");
            }
            try (Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT v,d FROM probe_adapter_write_probe WHERE id=1")) {
                if(!rows.next() || !Objects.equals(rows.getObject(1),"-00:00:00.000001") || rows.wasNull() || !Objects.equals(rows.getObject("d"),"1582-10-05") || rows.wasNull())throw new AssertionError("exact SQLType roundtrip");
                if(rows.getStatement()!=statement || statement.getConnection()!=connection)throw new AssertionError("statement chain");
            }
            try (PreparedStatement statement=connection.prepareStatement("UPDATE probe_adapter_write_probe SET v=?,d=? WHERE id=1")) {
                statement.setObject(1,null,Types.TIME);statement.setObject(2,null,Types.DATE);statement.executeUpdate();
            }
            try (Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT v,d FROM probe_adapter_write_probe WHERE id=1")) {
                if(!rows.next() || Objects.nonNull(rows.getObject(1)) || !rows.wasNull() || Objects.nonNull(rows.getObject(2)) || !rows.wasNull())throw new AssertionError("SQL NULL/wasNull");
            }
            boolean originalException=false;
            try (Statement statement=connection.createStatement()) {statement.executeQuery("SELECT * FROM probe_adapter_missing_table");}
            catch(java.sql.SQLException failure) {originalException=failure.getErrorCode()==1146 && Objects.equals(failure.getSQLState(),"42S02");}
            if(!originalException)throw new AssertionError("original SQL exception state");
        } finally { connection.rollback();connection.setAutoCommit(autoCommit); }
        try (Statement statement=connection.createStatement();ResultSet rows=statement.executeQuery("SELECT COUNT(*) FROM probe_adapter_write_probe")) {
            if(!rows.next() || rows.getLong(1)!=0)throw new AssertionError("rollback escaped connection owner");
        }
    }

    private static ResultSet exactRows(ResultSet rows, String mode) {
        return (ResultSet) Proxy.newProxyInstance(ResultSet.class.getClassLoader(), new Class<?>[]{ResultSet.class}, (proxy, method, args) -> {
            if ("getObject".equals(method.getName()) && Objects.nonNull(args) && args.length == 1) {
                int index = args[0] instanceof Integer ? (Integer) args[0] : rows.findColumn((String) args[0]);
                int jdbc = rows.getMetaData().getColumnType(index);
                if (jdbc == Types.DATE || jdbc == Types.TIME) return mode.startsWith("read_bytes") ? ExactTemporalValueReader.read(rows, index) : rows.getString(index);
            }
            try { return method.invoke(rows, args); }
            catch (InvocationTargetException failure) { throw failure.getCause(); }
        });
    }

    private static Object value(TableRecords records) {
        if (records.getRows().size() != 1) throw new AssertionError("row count");
        Row row = records.getRows().get(0);
        for (Field field : row.getFields()) if ("v".equals(field.getName())) return field.getValue();
        throw new AssertionError("missing value field");
    }

    private static Map<String, Object> describe(Object value) {
        Map<String, Object> result = new LinkedHashMap<>();
        result.put("class", Objects.isNull(value) ? null : value.getClass().getName());
        result.put("text", Objects.isNull(value) ? null : value.toString());
        if (value instanceof Date) result.put("epoch_millis", ((Date) value).getTime());
        if (value instanceof java.sql.Timestamp) result.put("nanoseconds", ((java.sql.Timestamp) value).getNanos());
        return result;
    }
}
