// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.mysql.cj.jdbc.MysqlDataSource;
import io.seatarust.compat.mysql.JavaAtWriterAdmission;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;
import lombok.extern.slf4j.Slf4j;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.config.ConfigurationFactory;
import org.apache.seata.rm.RMClient;
import org.apache.seata.rm.datasource.ConnectionProxy;
import org.apache.seata.rm.datasource.DataSourceProxy;
import org.apache.seata.rm.datasource.exec.LockConflictException;
import org.apache.seata.rm.datasource.exec.LockWaitTimeoutException;
import org.apache.seata.tm.TMClient;
import org.apache.seata.tm.api.GlobalTransaction;
import org.apache.seata.tm.api.GlobalTransactionContext;
import org.springframework.mock.env.MockEnvironment;
import org.springframework.util.StringUtils;

/** 真实Java AT阶段一持锁参与者。对应Java: DataSourceProxy、GlobalTransaction。 */
@Slf4j
public final class SharedLockJavaActor {
    private static final ObjectMapper JSON = new ObjectMapper();

    /** 从唯一自有目录启动原客户端，等待外部只读观察后提交；参数为D/E目录，无返回值。 */
    public static void main(String[] args) throws Exception {
        if (Objects.isNull(args) || args.length != 1) throw new IllegalArgumentException("One owned directory required");
        Path work = Path.of(args[0]).toAbsolutePath().normalize();
        if (!(work.toString().startsWith("D:\\") || work.toString().startsWith("E:\\"))) throw new IllegalArgumentException("D/E only");
        JsonNode cfg = JSON.readTree(Files.readAllBytes(work.resolve("database.json")));
        MockEnvironment env = new MockEnvironment().withProperty("seata.config.type", "file")
                .withProperty("seata.config.file.name", "file").withProperty("seata.registry.type", "file");
        ObjectHolder.INSTANCE.setObject(Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT, env);
        System.setProperty("service.vgroupMapping.cross_lock", "default");
        String endpoint = System.getProperty("fixture.tc.endpoint", "127.0.0.1:28094");
        System.setProperty("service.default.grouplist", endpoint);
        System.setProperty("transport.enableTmClientBatchSendRequest", "false");
        System.setProperty("transport.enableRmClientBatchSendRequest", "false");
        boolean expectConflict = cfg.path("expect_lock_conflict").asBoolean();
        String keyShape = cfg.path("account_key").asText(cfg.path("composite_key").asBoolean()
                ? "composite" : "integer");
        String predicate = switch (keyShape) {
            case "integer" -> "id = 1";
            case "composite" -> "id = 9007199254740993 AND segment = 'NORTH'";
            case "decimal-plain" -> "id = 9007199254740993.125";
            case "decimal-scientific" -> "id = 0.0000001";
            case "binary" -> "id = X'FF'";
            case "tinyint-boolean", "tinyint-mismatch" -> "id = 1";
            case "tinyint-numeric" -> "id = 2";
            default -> throw new IllegalArgumentException("Unexpected account key shape");
        };
        String updateSql = "UPDATE accounts SET balance = balance - 100 WHERE " + predicate;
        if (expectConflict) {
            System.setProperty("client.rm.lock.retryTimes", "0");
            if (ConfigurationFactory.getInstance().getInt("client.rm.lock.retryTimes", -1) != 0) {
                throw new IllegalStateException("Owned Java contender retry budget differs");
            }
        }
        if (!endpoint.equals(ConfigurationFactory.getInstance().getConfig("service.default.grouplist"))) throw new IllegalStateException("Owned TC route differs");
        TMClient.init("shared-lock-java", "cross_lock");
        RMClient.init("shared-lock-java", "cross_lock");
        MysqlDataSource direct = new MysqlDataSource();
        direct.setURL(cfg.path("jdbc_url").asText());direct.setUser(cfg.path("user").asText());direct.setPassword(cfg.path("password").asText());
        DataSourceProxy proxy = new DataSourceProxy(direct);
        GlobalTransaction transaction = GlobalTransactionContext.getCurrentOrCreate();
        JsonNode admission = cfg.path("writer_admission");
        String readSql = "SELECT balance FROM accounts WHERE " + predicate;
        try (ConnectionProxy connection = proxy.getConnection()) {
            if (admission.isObject() || expectConflict) {
                connection.setAutoCommit(false);
            }
            if (admission.isObject() && !admit(work, connection, admission)) {
                System.exit(0);
                return;
            }
            transaction.begin(180000, "shared-lock-java-held");
            if (expectConflict) {
                observeRejectedCommit(work, connection, transaction, updateSql, readSql);
            } else {
                executeBusinessUpdate(connection, updateSql);
                if (admission.isObject()) {
                    connection.commit();
                }
            }
        }
        if (expectConflict) {
            awaitSignal(work.resolve("stop.ready"));
            System.exit(0);
            return;
        }
        String xid = transaction.getXid();
        if (!StringUtils.hasText(xid)) {
            throw new IllegalStateException("Original Begin missing XID");
        }
        Map<String,Object> phase = new LinkedHashMap<>();phase.put("xid", xid);phase.put("resource", proxy.getResourceId());phase.put("process_id", ProcessHandle.current().pid());
        try (Connection connection = direct.getConnection(); PreparedStatement query = connection.prepareStatement("SELECT branch_id,HEX(rollback_info) FROM undo_log WHERE xid=?")) {
            query.setString(1,xid);
            try (ResultSet rows = query.executeQuery()) {
                if (!rows.next()) throw new IllegalStateException("Original Java Undo absent");
                phase.put("branch_id", rows.getLong(1));phase.put("undo_hex",rows.getString(2));
                if (rows.next()) throw new IllegalStateException("Duplicate Java branch Undo");
            }
        }
        Files.write(work.resolve("phase-one.json"),JSON.writeValueAsBytes(phase),StandardOpenOption.CREATE_NEW);
        log.info("REAL_JAVA_PHASE_ONE xid={} branch={}",xid,phase.get("branch_id"));
        awaitSignal(work.resolve("release.ready"));
        transaction.commit();
        Files.writeString(work.resolve("finished.ready"),xid,StandardOpenOption.CREATE_NEW);
        log.info("REAL_JAVA_GLOBAL_COMMIT xid={}",xid);
        // 保持真实RM可处理异步AT清理；测试控制器在Undo清空/原TC终态确认后才停此进程。
        awaitSignal(work.resolve("stop.ready"));
        System.exit(0);
    }

    /** 在原手动事务中检查锁域绑定，等待观察端核对事务持有的共享锁。 */
    private static boolean admit(Path work, ConnectionProxy connection, JsonNode admission) throws Exception {
        try {
            JavaAtWriterAdmission.verify(connection, UUID.fromString(admission.path("server_uuid").asText()),
                    admission.path("schema").asText());
        } catch (SQLException error) {
            if (!"25000".equals(error.getSQLState())) {
                throw error;
            }
            connection.rollback();
            Files.write(work.resolve("admission-rejected.json"),
                    JSON.writeValueAsBytes(Map.of("reason", error.getMessage(), "process_id", ProcessHandle.current().pid())),
                    StandardOpenOption.CREATE_NEW);
            awaitSignal(work.resolve("stop.ready"));
            return false;
        }
        Files.writeString(work.resolve("admission-confirmed"), "Binding verified", StandardOpenOption.CREATE_NEW);
        awaitSignal(work.resolve("admission-release"));
        return true;
    }

    private static void executeBusinessUpdate(Connection connection, String sql) throws SQLException {
        try (PreparedStatement update = connection.prepareStatement(sql)) {
            if (update.executeUpdate() != 1) {
                throw new IllegalStateException("Actual Java business row differs");
            }
        }
    }

    /** 原 JDBC 提交拒绝时，核对同一连接内的业务变化已经由 Java 客户端撤销。 */
    private static void observeRejectedCommit(Path work, ConnectionProxy connection, GlobalTransaction tx,
                                              String updateSql, String readSql) throws Exception {
        Map<String, Object> result = new LinkedHashMap<>();
        result.put("xid", tx.getXid());
        result.put("resource", connection.getDataSourceProxy().getResourceId());
        result.put("process_id", ProcessHandle.current().pid());
        try (PreparedStatement update = connection.prepareStatement(updateSql);
             PreparedStatement read = connection.prepareStatement(readSql)) {
            if (update.executeUpdate() != 1) {
                throw new IllegalStateException("Unexpected Java contender update count");
            }
            result.put("pending_balance", balance(read));
            try {
                connection.commit();
                throw new IllegalStateException("Java contender unexpectedly committed");
            } catch (LockConflictException conflict) {
                result.put("lock_conflict", conflict.getCode().name());
            } catch (LockWaitTimeoutException timeout) {
                if (!(timeout.getCause() instanceof LockConflictException conflict)) {
                    throw timeout;
                }
                result.put("lock_conflict", conflict.getCode().name());
            }
            result.put("balance_after_rejection", balance(read));
        }
        tx.rollback();
        Files.write(work.resolve("conflict.json"), JSON.writeValueAsBytes(result), StandardOpenOption.CREATE_NEW);
    }

    private static int balance(PreparedStatement read) throws Exception {
        try (ResultSet rows = read.executeQuery()) {
            if (!rows.next()) {
                throw new IllegalStateException("Business row absent");
            }
            return rows.getInt(1);
        }
    }

    private static void awaitSignal(Path signal) throws Exception {
        long deadline = System.nanoTime() + 120_000_000_000L;
        while (!Files.exists(signal)) {
            if (System.nanoTime() > deadline) {
                throw new IllegalStateException("Owned signal missing: " + signal.getFileName());
            }
            Thread.sleep(20);
        }
    }
}
