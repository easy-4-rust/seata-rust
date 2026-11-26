// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.mysql.cj.jdbc.MysqlDataSource;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import lombok.extern.slf4j.Slf4j;
import org.apache.seata.common.Constants;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.config.ConfigurationFactory;
import org.apache.seata.rm.RMClient;
import org.apache.seata.rm.datasource.DataSourceProxy;
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
        System.setProperty("service.default.grouplist", "127.0.0.1:28094");
        System.setProperty("transport.enableTmClientBatchSendRequest", "false");
        System.setProperty("transport.enableRmClientBatchSendRequest", "false");
        if (!"127.0.0.1:28094".equals(ConfigurationFactory.getInstance().getConfig("service.default.grouplist"))) throw new IllegalStateException("Owned TC route differs");
        TMClient.init("shared-lock-java", "cross_lock");
        RMClient.init("shared-lock-java", "cross_lock");
        MysqlDataSource direct = new MysqlDataSource();
        direct.setURL(cfg.path("jdbc_url").asText());direct.setUser(cfg.path("user").asText());direct.setPassword(cfg.path("password").asText());
        DataSourceProxy proxy = new DataSourceProxy(direct);
        GlobalTransaction transaction = GlobalTransactionContext.getCurrentOrCreate();
        transaction.begin(180000, "shared-lock-java-held");
        String xid = transaction.getXid();
        if (!StringUtils.hasText(xid)) throw new IllegalStateException("Original Begin missing XID");
        try (Connection connection = proxy.getConnection(); PreparedStatement update = connection.prepareStatement("UPDATE accounts SET balance = balance - 100 WHERE id = 1")) {
            if (update.executeUpdate() != 1) throw new IllegalStateException("Actual Java business row differs");
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
        long deadline = System.nanoTime()+120000000000L;
        while (!Files.exists(work.resolve("release.ready"))) {
            if (System.nanoTime()>deadline) throw new IllegalStateException("Owned release missing");
            Thread.sleep(20);
        }
        if ("rollback".equals(cfg.path("end_action").asText())) transaction.rollback(); else transaction.commit();
        Files.writeString(work.resolve("finished.ready"),xid,StandardOpenOption.CREATE_NEW);
        log.info("REAL_JAVA_GLOBAL_END xid={}",xid);
        // 保持真实RM可处理异步AT清理；测试控制器在Undo清空/原TC终态确认后才停此进程。
        while (!Files.exists(work.resolve("stop.ready"))) Thread.sleep(20);
        System.exit(0);
    }
}
