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
import com.alipay.sofa.jraft.entity.LocalStorageOutter.LocalSnapshotPbMeta;
import com.alipay.sofa.jraft.storage.io.ProtoBufFile;
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
public final class RaftBusinessControl {
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

    private static Map<String, String> query(String raw) {
        Map<String, String> values = new LinkedHashMap<>();
        if (Objects.nonNull(raw)) {
            for (String part : raw.split("&")) {
                String[] pair = part.split("=", 2);
                if (pair.length == 2) {
                    values.put(java.net.URLDecoder.decode(pair[0], StandardCharsets.UTF_8),
                            java.net.URLDecoder.decode(pair[1], StandardCharsets.UTF_8));
                }
            }
        }
        return values;
    }

    private static Map<String, Object> inspect(Map<String, String> query) throws Exception {
        RaftServer raft = RaftServerManager.getRaftServer("default");
        Node node = raft.getNode();
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("term_before", raft.getRaftStateMachine().getCurrentTerm().get());
        out.put("raft_term_before", actualRaftTerm(node));
        out.put("raft_leader_before", node.isLeader());
        out.put("raft_leader_id_before", node.getLeaderId().toString());
        out.put("raft_peer_id", node.getNodeId().getPeerId().toString());
        out.put("pid", ProcessHandle.current().pid());
        out.put("leader", raft.getRaftStateMachine().isLeader());
        out.put("leader_id", node.getLeaderId().toString());
        out.put("node_id", node.getNodeId().toString());
        out.put("term", raft.getRaftStateMachine().getCurrentTerm().get());
        out.put("reload_complete_term", raft.getRaftStateMachine().getSessionReloadCompletedTerm());
        out.put("committed", node.getLastCommittedIndex());
        out.put("applied", node.getLastAppliedLogIndex());
        out.put("node_state", node.getNodeState().toString());
        SessionManager manager = SessionHolder.getRootSessionManager("default");
        out.put("all_sessions", manager.allSessions().size());
        List<Map<String, Object>> records = new ArrayList<>();
        // 仅读取原会话与分支；不提供seed、状态变更或业务响应入口。
        for (GlobalSession session : manager.allSessions()) {
            Map<String, Object> record = new LinkedHashMap<>();
            record.put("xid", session.getXid());
            record.put("status", session.getStatus().getCode());
            record.put("active", session.isActive());
            record.put("begin_time", session.getBeginTime());
            record.put("application_id", session.getApplicationId());
            record.put("transaction_name", session.getTransactionName());
            record.put("transaction_service_group", session.getTransactionServiceGroup());
            record.put("application_data", session.getApplicationData());
            List<Map<String, Object>> branches = new ArrayList<>();
            for (BranchSession branch : session.getBranchSessions()) {
                Map<String, Object> row = new LinkedHashMap<>();
                row.put("branch_id", branch.getBranchId());
                row.put("xid", branch.getXid());
                row.put("type", branch.getBranchType().ordinal());
                row.put("status", branch.getStatus().getCode());
                row.put("resource", branch.getResourceId());
                row.put("lock_key", branch.getLockKey());
                row.put("application_data", branch.getApplicationData());
                row.put("client_id", branch.getClientId());
                branches.add(row);
            }
            record.put("branches", branches);
            records.add(record);
        }
        String resource = query.get("resource");
        out.put("lockable", Objects.isNull(resource) ? null : LockerManagerFactory.getLockManager()
                .isLockable("127.0.0.1:28291:999999", resource, "ACCOUNTS:1;"));
        out.put("records", records);
        Map<String, Object> snapshot = new LinkedHashMap<>();
        Path root = Path.of(System.getProperty("seata.it.raft.dir")).resolve("store/raft")
                .resolve(System.getProperty("server.raftPort")).resolve("default/snapshot");
        long latest = -1;
        if (Files.isDirectory(root)) {
            try (java.util.stream.Stream<Path> children = Files.list(root)) {
                for (Path child : (Iterable<Path>) children::iterator) {
                    Path metadata = child.resolve("__raft_snapshot_meta");
                    if (child.getFileName().toString().matches("snapshot_[0-9]+") && Files.isRegularFile(metadata)) {
                        LocalSnapshotPbMeta value = new ProtoBufFile(metadata.toString()).load();
                        long index = value.getMeta().getLastIncludedIndex();
                        if (index > latest) {
                            latest = index;
                            snapshot.put("directory", child.toRealPath().toString());
                            snapshot.put("index", index);
                            snapshot.put("term", value.getMeta().getLastIncludedTerm());
                        }
                    }
                }
            }
        }
        out.put("snapshot", snapshot);
        out.put("term_after", raft.getRaftStateMachine().getCurrentTerm().get());
        out.put("raft_term_after", actualRaftTerm(node));
        out.put("raft_leader_after", node.isLeader());
        out.put("raft_leader_id_after", node.getLeaderId().toString());
        return out;
    }

    /** 只读获取固定 JRaft 实现的实际任期，避免仅凭 Seata 回调字段宣称当前领导恢复完成。 */
    private static long actualRaftTerm(Node node) throws Exception {
        java.lang.reflect.Method method = node.getClass().getDeclaredMethod("getCurrentTerm");
        method.setAccessible(true);
        return (long) method.invoke(node);
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
                if ("/snapshot".equals(operation) && "POST".equals(exchange.getRequestMethod())) {
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
                answer = inspect(query(exchange.getRequestURI().getRawQuery()));
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
