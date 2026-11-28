// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package io.seatarust.compat.mysql;

import com.mysql.cj.conf.PropertyKey;
import com.mysql.cj.jdbc.JdbcConnection;
import org.apache.seata.rm.datasource.ConnectionProxy;

import java.nio.charset.StandardCharsets;
import java.lang.reflect.Field;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.time.Instant;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.util.Arrays;
import java.util.Objects;
import java.util.UUID;

/** 在固定 Java AT 客户端写入前核对原事务的 MySQL 锁域。对应 Java: ConnectionProxy。 */
public final class JavaAtWriterAdmission {
    private static final String BINDING = "seata_rs_at_resource_binding";
    private static final int QUERY_TIMEOUT_SECONDS = 3;

    private JavaAtWriterAdmission() {
    }

    /**
     * 校验将执行首条业务 SQL 的原 AT 连接及部署方预配的 UUID、schema。
     * 正常返回时，绑定行的共享锁及表元数据锁由原事务持有。校验失败后调用方回滚原事务。
     */
    public static void verify(ConnectionProxy connection, UUID expectedServer, String expectedSchema)
            throws SQLException {
        verifyBinding(connection, expectedServer, expectedSchema, null);
    }

    /**
     * 按稳定配置码 1–6 核验实际驱动、时间表示及 021 绑定，正常返回后才能执行业务 SQL。
     * 参数为原 AT 连接、部署 UUID/schema 和完整 JDBC 配置码；失败返回 SQLException，调用方须回滚。
     * 对应 Java: ConnectionProxy#getTargetConnection、JacksonUndoLogParser#setZoneOffset。
     */
    public static void verify(ConnectionProxy connection, UUID expectedServer, String expectedSchema, int jdbcProfile)
            throws SQLException {
        if (jdbcProfile < 1 || jdbcProfile > 6) {
            throw rejected("jdbc-profile");
        }
        verifyBinding(connection, expectedServer, expectedSchema, jdbcProfile);
    }

    private static void verifyBinding(ConnectionProxy connection, UUID expectedServer, String expectedSchema,
                                      Integer jdbcProfile) throws SQLException {
        Objects.requireNonNull(connection, "AT connection");
        Objects.requireNonNull(expectedServer, "MySQL server UUID");
        Objects.requireNonNull(expectedSchema, "MySQL schema");

        Connection target = connection.getTargetConnection();
        if (target.getAutoCommit()) {
            throw rejected("manual-transaction");
        }

        try (PreparedStatement statement = prepare(target,
                "SELECT @@server_uuid,DATABASE(),@@global.read_only,@@global.super_read_only");
             ResultSet rows = statement.executeQuery()) {
            if (!rows.next() || !expectedServer.toString().equals(rows.getString(1))
                    || !expectedSchema.equals(rows.getString(2)) || rows.getBoolean(3)
                    || rows.getBoolean(4)) {
                throw rejected("physical-schema");
            }
        }

        JdbcConnection jdbc = target.unwrap(JdbcConnection.class);
        boolean booleanMapping = jdbc.getPropertySet().getBooleanProperty(PropertyKey.tinyInt1isBit).getValue();
        if (Objects.nonNull(jdbcProfile)) {
            verifyRepresentation(connection, jdbc, booleanMapping, jdbcProfile);
        }
        String resource = connection.getDataSourceProxy().getResourceId();
        if (Objects.isNull(resource) || resource.isEmpty()) {
            throw rejected("jdbc-resource");
        }
        try (PreparedStatement statement = prepare(target,
                "SELECT resource_id,tinyint1_is_bit FROM " + BINDING + " WHERE slot=1 LIMIT 2 FOR SHARE");
             ResultSet rows = statement.executeQuery()) {
            if (!rows.next() || !Arrays.equals(rows.getBytes(1), resource.getBytes(StandardCharsets.UTF_8))) {
                throw rejected("jdbc-resource");
            }
            String mapped = rows.getString(2);
            if (!(booleanMapping ? "1" : "0").equals(mapped)) {
                throw rejected("tinyint-profile");
            }
            if (rows.next()) {
                throw rejected("jdbc-resource");
            }
        }

        if (Objects.nonNull(jdbcProfile)) {
            try (PreparedStatement statement = prepare(target,
                    "SELECT jdbc_profile FROM " + BINDING + " WHERE slot=1 LIMIT 2 FOR SHARE");
                 ResultSet rows = statement.executeQuery()) {
                if (!rows.next() || !jdbcProfile.toString().equals(rows.getString(1)) || rows.next()) {
                    throw rejected("jdbc-profile");
                }
            }
        }

        // 在原连接已经持有表元数据锁后，检查当前对象确实提供 InnoDB 行锁。
        try (PreparedStatement statement = prepare(target, "SHOW CREATE TABLE " + BINDING);
             ResultSet rows = statement.executeQuery()) {
            if (!rows.next() || !BINDING.equals(rows.getString(1))) {
                throw rejected("binding-table");
            }
            String ddl = rows.getString(2);
            if (Objects.isNull(ddl) || !ddl.startsWith("CREATE TABLE ")
                    || !ddl.contains("\n) ENGINE=InnoDB ")) {
                throw rejected("binding-table");
            }
        }
    }

    private static void verifyRepresentation(ConnectionProxy connection, JdbcConnection jdbc,
                                             boolean booleanMapping, int profile) throws SQLException {
        String version = connection.getTargetConnection().getMetaData().getDriverVersion();
        if (Objects.isNull(version) || !version.startsWith("mysql-connector-java-8.0.27 ")) {
            throw rejected("jdbc-driver");
        }
        if (booleanMapping != (profile % 2 == 1)) {
            throw rejected("tinyint-profile");
        }
        // 避免旧三参数应用必须链接新装饰器；新精确模式必须能解析到实际类型。
        boolean exact = false;
        try {
            Class<?> adapter = Class.forName("io.github.easy4j.seata.rust.jdbc.ExactTemporalDataSource",
                    false, JavaAtWriterAdmission.class.getClassLoader());
            exact = adapter.isInstance(connection.getDataSourceProxy().getTargetDataSource());
        } catch (ClassNotFoundException absent) {
            if (profile >= 5) {
                throw rejected("exact-temporal-profile");
            }
        }
        if (exact != (profile >= 5)) {
            throw rejected("exact-temporal-profile");
        }
        if (profile >= 3) {
            if (!"UTC".equals(jdbc.getPropertySet().getStringProperty(PropertyKey.connectionTimeZone).getValue())
                    || !isUtc(ZoneId.systemDefault()) || !isUtc(undoZone())) {
                throw rejected("utc-profile");
            }
            try (PreparedStatement statement = prepare(connection.getTargetConnection(), "SELECT @@session.time_zone");
                 ResultSet rows = statement.executeQuery()) {
                if (!rows.next() || !"+00:00".equals(rows.getString(1))) {
                    throw rejected("utc-profile");
                }
            }
        }
    }

    private static boolean isUtc(ZoneId zone) {
        return Objects.nonNull(zone) && zone.getRules().isFixedOffset()
                && ZoneOffset.UTC.equals(zone.getRules().getOffset(Instant.EPOCH));
    }

    private static ZoneId undoZone() throws SQLException {
        // 固定原版本没有公开 getter；只读原字段，禁止在每次准入时改写进程级 parser 配置。
        try {
            Class<?> parser = Class.forName("org.apache.seata.rm.datasource.undo.parser.JacksonUndoLogParser");
            Field field = parser.getDeclaredField("zoneId");
            field.setAccessible(true);
            return (ZoneId) field.get(null);
        } catch (ReflectiveOperationException | RuntimeException error) {
            SQLException failure = rejected("undo-timezone");
            failure.initCause(error);
            throw failure;
        }
    }

    private static PreparedStatement prepare(Connection target, String sql) throws SQLException {
        PreparedStatement statement = target.prepareStatement(sql);
        try {
            statement.setQueryTimeout(QUERY_TIMEOUT_SECONDS);
            return statement;
        } catch (SQLException error) {
            try {
                statement.close();
            } catch (SQLException closingError) {
                error.addSuppressed(closingError);
            }
            throw error;
        }
    }

    private static SQLException rejected(String reason) {
        return new SQLException(reason, "25000");
    }
}
