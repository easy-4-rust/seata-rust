/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements. See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
package org.apache.seata.server.session;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.seata.common.XID;
import org.apache.seata.common.holder.ObjectHolder;
import org.apache.seata.common.store.SessionMode;
import org.apache.seata.config.Configuration;
import org.apache.seata.config.ConfigurationFactory;
import org.apache.seata.core.model.BranchStatus;
import org.apache.seata.core.model.BranchType;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.server.lock.LockerManagerFactory;
import org.apache.seata.server.store.StoreConfig;
import org.springframework.core.env.MapPropertySource;
import org.springframework.core.env.StandardEnvironment;
import org.springframework.util.StringUtils;

import java.lang.management.ManagementFactory;
import java.nio.file.Paths;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

import static org.apache.seata.common.Constants.OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT;

/** 真实 DB/Redis 重试超时持久化夹具；不是生产 TC 补丁。 */
public final class BackendRetryTimeoutStoreProbe {
    private BackendRetryTimeoutStoreProbe() {}

    static String fixtureClientId() {
        return "backend-retry-probe:127.0.0.1:1";
    }

    private static void flatten(JsonNode node, String prefix, Map<String, Object> values) {
        if (node.isObject()) {
            node.fields()
                    .forEachRemaining(entry -> flatten(
                            entry.getValue(),
                            StringUtils.hasText(prefix) ? prefix + "." + entry.getKey() : entry.getKey(),
                            values));
        } else if (node.isValueNode() && !node.isNull()) {
            values.put(prefix, node.asText());
        } else {
            throw new IllegalArgumentException("Fixture configuration must contain only scalar leaves");
        }
    }

    private static void prepareEnvironment(String backend) throws Exception {
        String fixture = System.getProperty("seata.it.fixture.config");
        if (!StringUtils.hasText(fixture) || !fixture.matches("^[DE]:[/\\\\].*")) {
            throw new IllegalArgumentException("An owned D/E fixture configuration is required");
        }
        Map<String, Object> values = new LinkedHashMap<>();
        flatten(new ObjectMapper().readTree(Paths.get(fixture).toFile()), "", values);
        // 原 JAR 的 Spring 配置 SPI 需要真实 Environment；在首次读取 SPI 前验证独立测试端点。
        if ("db".equals(backend)) {
            if (!String.valueOf(values.get("store.db.url")).startsWith("jdbc:mysql://127.0.0.1:28093/seata_retry_")) {
                throw new IllegalArgumentException("Only the isolated MySQL fixture endpoint is permitted");
            }
        } else if (!"127.0.0.1".equals(values.get("store.redis.single.host"))
                || !"28094".equals(values.get("store.redis.single.port"))
                || !"0".equals(values.get("store.redis.database"))) {
            throw new IllegalArgumentException("Only the isolated Redis fixture endpoint is permitted");
        }
        Map<String, Object> properties = new LinkedHashMap<>();
        values.forEach((key, value) -> properties.put("seata." + key, value));
        properties.put("seata.config.type", "file");
        properties.put("seata.config.file.name", "file:" + fixture.replace('\\', '/'));
        StandardEnvironment environment = new StandardEnvironment();
        environment.getPropertySources().addFirst(new MapPropertySource("owned-retry-timeout-fixture", properties));
        ObjectHolder.INSTANCE.setObject(OBJECT_KEY_SPRING_CONFIGURABLE_ENVIRONMENT, environment);
    }

    /** 输入 seed/inspect 与 db/redis；仅访问独立本地测试端点。 */
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected seed/inspect and db/redis");
        }
        if ((!"seed".equals(args[0]) && !"inspect".equals(args[0]))
                || (!"db".equals(args[1]) && !"redis".equals(args[1]))) {
            throw new IllegalArgumentException("Unsupported backend fixture mode/store");
        }
        String backend = args[1];
        prepareEnvironment(backend);
        Configuration config = ConfigurationFactory.getInstance();
        if ("db".equals(backend)) {
            String url = config.getConfig("store.db.url");
            if (!StringUtils.hasText(url) || !url.startsWith("jdbc:mysql://127.0.0.1:28093/seata_retry_")) {
                throw new IllegalArgumentException("Only the isolated MySQL fixture endpoint is permitted");
            }
        } else if (!"127.0.0.1".equals(config.getConfig("store.redis.single.host"))
                || config.getInt("store.redis.single.port") != 28094
                || config.getInt("store.redis.database") != 0) {
            throw new IllegalArgumentException("Only the isolated Redis fixture endpoint is permitted");
        }
        if (!backend.equals(StoreConfig.getSessionMode().getName())
                || !backend.equals(StoreConfig.getLockMode().getName())) {
            throw new IllegalArgumentException("Session and lock modes must match the fixture backend");
        }
        XID.setIpAddress("127.0.0.1");
        XID.setPort(28091);
        SessionHolder.init(SessionMode.get(backend));
        LockerManagerFactory.init();
        System.out.println("PROCESS mode=" + args[0] + " backend=" + backend + " identity="
                + ManagementFactory.getRuntimeMXBean().getName());
        try {
            SessionManager manager = SessionHolder.getRootSessionManager();
            for (int index = 0; index < 4; index++) {
                int status = index % 2 == 0 ? 16 : 17;
                boolean withBranch = index < 2;
                String xid = "127.0.0.1:28091:" + (920000 + index);
                String resource = "mysql://backend-retry-" + backend + "-" + index;
                if ("seed".equals(args[0])) {
                    if (Objects.nonNull(manager.findGlobalSession(xid))) {
                        throw new IllegalStateException("Refusing to overwrite an existing retry-timeout session");
                    }
                    GlobalSession session =
                            new GlobalSession("backend-retry-probe", "default_tx_group", "retry-timeout", 600000);
                    session.setTransactionId(920000 + index);
                    session.setXid(xid);
                    session.setBeginTime(System.currentTimeMillis());
                    manager.addGlobalSession(session);
                    if (withBranch) {
                        BranchSession branch = new BranchSession();
                        branch.setTransactionId(session.getTransactionId());
                        branch.setXid(xid);
                        branch.setBranchId(930000 + index);
                        branch.setBranchType(BranchType.AT);
                        branch.setStatus(BranchStatus.PhaseOne_Done);
                        branch.setResourceId(resource);
                        branch.setLockKey("orders:1");
                        branch.setClientId(fixtureClientId());
                        branch.setApplicationData("{}");
                        if (!branch.lock()) {
                            throw new IllegalStateException("Seed branch failed to acquire an actual stored lock");
                        }
                        session.add(branch);
                        manager.addBranchSession(session, branch);
                    }
                    manager.updateGlobalSessionStatus(session, GlobalStatus.get(status));
                }
                GlobalSession restored = manager.findGlobalSession(xid, true);
                if (Objects.isNull(restored)
                        || restored.getStatus().getCode() != status
                        || restored.getBranchSessions().size() != (withBranch ? 1 : 0)
                        || (withBranch && Objects.isNull(restored.getBranch(930000L + index)))) {
                    throw new IllegalStateException("Actual backend record identity/status/branch count differs");
                }
                boolean lockable = LockerManagerFactory.getLockManager()
                        .isLockable("127.0.0.1:28091:999999", resource, "orders:1");
                if (lockable == withBranch) {
                    throw new IllegalStateException("Actual backend lock occupation differs");
                }
                System.out.println("RECORD backend=" + backend + " index=" + index + " xid=" + xid + " status=" + status
                        + " branches=" + restored.getBranchSessions().size() + " branchId="
                        + (withBranch ? 930000 + index : 0)
                        + " resource=" + resource + " lockable=" + lockable);
            }
        } finally {
            SessionHolder.destroy();
            LockerManagerFactory.destroy();
        }
        // 测试主入口在持久化复查后结束自有 JVM，避免池线程掩盖进程切换；不是业务服务退出策略。
        System.exit(0);
    }
}
