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
package org.apache.seata.server.cluster.raft;

import com.alipay.sofa.jraft.Node;
import com.alipay.sofa.jraft.Status;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.sun.net.httpserver.HttpServer;
import jdk.jfr.Recording;
import org.apache.seata.common.store.SessionMode;
import org.apache.seata.core.model.BranchStatus;
import org.apache.seata.core.model.BranchType;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.server.ServerApplication;
import org.apache.seata.server.cluster.raft.context.SeataClusterContext;
import org.apache.seata.server.lock.LockerManagerFactory;
import org.apache.seata.server.session.BranchSession;
import org.apache.seata.server.session.GlobalSession;
import org.apache.seata.server.session.SessionHolder;
import org.apache.seata.server.session.SessionManager;
import org.apache.seata.server.store.StoreConfig;
import org.springframework.util.StringUtils;

import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

/** 原 TC 外挂的现场测试控制入口；不替换原事务／复制／恢复实现。 */
public final class RaftRetryTimeoutControl {
    /** 校验 D/E 自有目录、标记与保留控制端口；参数为目录、端口、运行归属，非法时拒绝。 */
    public static void validateFixture(Path directory, int port, String token) throws Exception {
        if (Objects.isNull(directory) || !StringUtils.hasText(token) || !token.matches("phase0-raft-[a-f0-9]{8}")) {
            throw new IllegalArgumentException("Missing valid owned RAFT fixture identity");
        }
        if (!directory.toAbsolutePath().normalize().toString().matches("^[DE]:[\\\\/].*")
                || port < 28101
                || port > 28103) {
            throw new IllegalArgumentException("RAFT fixture must stay on D/E and reserved control ports");
        }
        Path marker = directory.resolve(".phase0-owned-raft");
        if (!Files.isRegularFile(marker)
                || !token.equals(Files.readString(marker).trim())
                || !directory.toRealPath().toString().matches("^[DE]:[\\\\/].*")) {
            throw new IllegalArgumentException("RAFT fixture ownership marker differs");
        }
    }

    private static String xid(int index) {
        return System.getProperty("seata.it.raft.host") + ":28091:" + (940000 + index);
    }

    /** 返回独立锁观察者身份，数字事务段须可由原 XID 工具解析。 */
    static String observerXid() {
        return System.getProperty("seata.it.raft.host") + ":28091:999999";
    }

    private static Map<String, Object> inspect() throws Exception {
        RaftServer raft = RaftServerManager.getRaftServer("default");
        Node node = raft.getNode();
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("pid", ProcessHandle.current().pid());
        out.put("leader", raft.getRaftStateMachine().isLeader());
        out.put("leader_id", node.getLeaderId().toString());
        out.put("node_id", node.getNodeId().toString());
        out.put("term", raft.getRaftStateMachine().getCurrentTerm().get());
        out.put("committed", node.getLastCommittedIndex());
        out.put("applied", node.getLastAppliedLogIndex());
        out.put("node_state", node.getNodeState().toString());
        SessionManager manager = SessionHolder.getRootSessionManager("default");
        out.put("all_sessions", manager.allSessions().size());
        List<Map<String, Object>> records = new ArrayList<>();
        for (int index = 0; index < 2; index++) {
            Map<String, Object> record = new LinkedHashMap<>();
            GlobalSession session = manager.findGlobalSession(xid(index), true);
            record.put("xid", xid(index));
            record.put("exists", Objects.nonNull(session));
            record.put("resource", "mysql://raft-retry-" + index);
            record.put(
                    "lockable",
                    LockerManagerFactory.getLockManager()
                            .isLockable(observerXid(), "mysql://raft-retry-" + index, "orders:1"));
            if (Objects.nonNull(session)) {
                record.put("xid", session.getXid());
                record.put("status", session.getStatus().getCode());
                record.put("branches", session.getBranchSessions().size());
                BranchSession branch = session.getBranch(950000L + index);
                record.put("branch_id", Objects.isNull(branch) ? 0L : branch.getBranchId());
                record.put("branch_resource", Objects.isNull(branch) ? null : branch.getResourceId());
                record.put("branch_lock_key", Objects.isNull(branch) ? null : branch.getLockKey());
                record.put("branch_xid", Objects.isNull(branch) ? null : branch.getXid());
                record.put(
                        "branch_type",
                        Objects.isNull(branch) ? -1 : branch.getBranchType().ordinal());
            }
            records.add(record);
        }
        out.put("records", records);
        return out;
    }

    private static void seed(int status) throws Exception {
        if ((status != 1 && status != 16 && status != 17) || !RaftServerManager.isLeader("default")) {
            throw new IllegalArgumentException("Seed requires owned leader and explicit diagnostic status");
        }
        SessionManager manager = SessionHolder.getRootSessionManager("default");
        for (int index = 0; index < 2; index++) {
            if (Objects.nonNull(manager.findGlobalSession(xid(index)))) {
                throw new IllegalArgumentException("Refusing to overwrite original fixture identity");
            }
        }
        for (int index = 0; index < 2; index++) {
            GlobalSession session = new GlobalSession("raft-retry-probe", "default_tx_group", "raft-retry", 600000);
            session.setTransactionId(940000 + index);
            session.setXid(xid(index));
            // 使用原生命周期提交 JRaft 任务；不写内部会话 map 或磁盘字节。
            session.begin();
            if (index == 0) {
                BranchSession branch = new BranchSession();
                branch.setTransactionId(session.getTransactionId());
                branch.setXid(session.getXid());
                branch.setBranchId(950000L + index);
                branch.setBranchType(BranchType.AT);
                branch.setStatus(BranchStatus.Registered);
                branch.setResourceId("mysql://raft-retry-" + index);
                branch.setLockKey("orders:1");
                branch.setClientId("raft-retry-probe:127.0.0.1:1");
                branch.setApplicationData("{}");
                if (!branch.lock()) {
                    throw new IllegalStateException("Original fixture lock was not acquired");
                }
                session.addBranch(branch);
            }
            session.changeGlobalStatus(GlobalStatus.get(status));
        }
    }

    /** 在归属校验后开启 JDK 异常记录并运行原应用；参数透传给原 ServerApplication。 */
    public static void main(String[] args) throws Exception {
        Path directory = Path.of(System.getProperty("seata.it.raft.dir"));
        int port = Integer.parseInt(System.getProperty("seata.it.raft.controlPort"));
        String token = System.getProperty("seata.it.raft.token");
        validateFixture(directory, port, token);
        Recording recording = new Recording();
        recording.enable("jdk.JavaExceptionThrow").withStackTrace();
        recording.start();
        ServerApplication.main(args);
        if (!RaftServerManager.isRaftMode() || StoreConfig.getSessionMode() != SessionMode.RAFT) {
            throw new IllegalStateException("Original TC is not using actual RAFT storage");
        }
        ObjectMapper mapper = new ObjectMapper();
        HttpServer control = HttpServer.create(new InetSocketAddress("127.0.0.1", port), 0);
        control.createContext("/", exchange -> {
            int code = 200;
            Map<String, Object> answer;
            SeataClusterContext.bindGroup("default");
            try {
                if (!token.equals(exchange.getRequestHeaders().getFirst("X-Phase0-Owner"))) {
                    throw new IllegalArgumentException("Wrong owned test process identity");
                }
                String operation = exchange.getRequestURI().getPath();
                if ("/seed".equals(operation) && "POST".equals(exchange.getRequestMethod())) {
                    seed(Integer.parseInt(exchange.getRequestURI().getQuery()));
                } else if ("/snapshot".equals(operation) && "POST".equals(exchange.getRequestMethod())) {
                    CompletableFuture<Status> completion = new CompletableFuture<>();
                    RaftServerManager.getRaftServer("default").getNode().snapshot(completion::complete);
                    Status status = completion.get(15, TimeUnit.SECONDS);
                    if (!status.isOk()) {
                        throw new IllegalStateException("Real JRaft snapshot failed: " + status);
                    }
                } else if ("/dump".equals(operation) && "POST".equals(exchange.getRequestMethod())) {
                    String label = exchange.getRequestURI().getQuery();
                    if (Objects.isNull(label) || !label.matches("[a-z_]{1,30}")) {
                        throw new IllegalArgumentException("Invalid owned JFR dump label");
                    }
                    Path dump = directory.resolve(label + ".jfr");
                    if (Files.exists(dump)) {
                        throw new IllegalArgumentException("Refusing to overwrite recorded JFR evidence");
                    }
                    recording.dump(dump);
                } else if (!"/state".equals(operation) || !"GET".equals(exchange.getRequestMethod())) {
                    throw new IllegalArgumentException("Unsupported owned diagnostic operation");
                }
                answer = inspect();
            } catch (Exception error) {
                code = 500;
                answer = new LinkedHashMap<>();
                answer.put("error", error.getClass().getName());
                answer.put("message", error.getMessage());
            } finally {
                SeataClusterContext.unbindGroup();
            }
            byte[] response = mapper.writeValueAsString(answer).getBytes(StandardCharsets.UTF_8);
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(code, response.length);
            exchange.getResponseBody().write(response);
            exchange.close();
        });
        control.start();
        System.out.println("RAFT_CONTROL_READY pid=" + ProcessHandle.current().pid() + " port=" + port);
    }
}
