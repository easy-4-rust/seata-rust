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

import org.apache.seata.common.XID;
import org.apache.seata.common.exception.ShouldNeverHappenException;
import org.apache.seata.common.store.SessionMode;
import org.apache.seata.core.model.BranchStatus;
import org.apache.seata.core.model.BranchType;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.server.storage.file.session.FileSessionManager;

import java.io.File;
import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Objects;

/** 固定 Java FILE 持久化探针入口；仅测试工具，不是 TC 生产补丁。 */
public final class FileRetryTimeoutStoreProbe {
    private FileRetryTimeoutStoreProbe() {}

    /** 接收模式、独立目录和状态码，执行子进程持久化或恢复检查。 */
    public static void main(String[] args) throws Exception {
        if (args.length != 3) {
            throw new IllegalArgumentException("Expected mode, fresh D/E directory, status");
        }
        String mode = args[0];
        int code = Integer.parseInt(args[2]);
        if ((!"seed".equals(mode) && !"inspect".equals(mode)) || (code != 1 && code != 16 && code != 17)) {
            throw new IllegalArgumentException("Only seed/inspect and statuses1/16/17 are supported");
        }
        Path directory = Paths.get(args[1]).toAbsolutePath().normalize();
        if (File.separatorChar == '\\' && !directory.toString().matches("^[DE]:\\\\.*")) {
            throw new IllegalArgumentException("Store must remain on D: or E:");
        }
        XID.setIpAddress("127.0.0.1");
        XID.setPort(28091);
        String xid = "127.0.0.1:28091:" + (900000 + code);
        System.out.println("PROCESS mode=" + mode + " identity="
                + ManagementFactory.getRuntimeMXBean().getName());
        if ("seed".equals(mode)) {
            seed(directory, code, xid);
        } else {
            inspect(directory, code, xid);
        }
    }

    private static void seed(Path directory, int code, String xid) throws Exception {
        if (Files.exists(directory)) {
            throw new IllegalArgumentException("Refusing existing store directory: " + directory);
        }
        Files.createDirectories(directory);
        FileSessionManager manager = new FileSessionManager("root.data", directory.toString());
        GlobalSession session = new GlobalSession("timeout-file-probe", "default_tx_group", "retry-timeout", 600000);
        session.setTransactionId(900000 + code);
        session.setXid(xid);
        session.setBeginTime(System.currentTimeMillis());
        BranchSession branch = new BranchSession();
        branch.setTransactionId(session.getTransactionId());
        branch.setXid(xid);
        branch.setBranchId(910000 + code);
        branch.setBranchType(BranchType.AT);
        branch.setStatus(BranchStatus.PhaseOne_Done);
        branch.setResourceId("mysql://timeout-file");
        branch.setLockKey("orders:1");
        branch.setClientId("timeout-file-probe:127.0.0.1:0");
        branch.setApplicationData("{}");
        try {
            // 通过真实存储管理器写入 Begin、分支和状态更新，不手工拼接或修改 FILE 字节。
            manager.addGlobalSession(session);
            session.add(branch);
            manager.addBranchSession(session, branch);
            manager.updateGlobalSessionStatus(session, GlobalStatus.get(code));
        } finally {
            manager.destroy();
        }
        if (Files.size(directory.resolve("root.data")) == 0) {
            throw new IllegalStateException("Writer produced no persisted bytes");
        }
        System.out.println("SEEDED status=" + session.getStatus().getCode() + " branches="
                + session.getBranchSessions().size() + " xid=" + xid + " branchId=" + branch.getBranchId());
    }

    private static void inspect(Path directory, int code, String xid) throws Exception {
        Path data = directory.resolve("root.data");
        if (!Files.isRegularFile(data) || Files.size(data) == 0) {
            throw new IllegalArgumentException("Missing persisted store: " + data);
        }
        FileSessionManager manager = new FileSessionManager("root.data", directory.toString());
        try {
            manager.reload();
            GlobalSession restored = manager.findGlobalSession(xid);
            int branches = manager.allSessions().stream()
                    .mapToInt(session -> session.getBranchSessions().size())
                    .sum();
            System.out.println(
                    "RESTORED sessions=" + manager.allSessions().size() + " branches=" + branches + " status="
                            + (Objects.isNull(restored)
                                    ? "absent"
                                    : restored.getStatus().getCode()));
            if (Objects.nonNull(restored)) {
                if (restored.getStatus().getCode() != code || Objects.isNull(restored.getBranch(910000L + code))) {
                    throw new IllegalStateException("Restored identity or status differs from persisted fixture");
                }
                try {
                    SessionHolder.reload(manager.allSessions(), SessionMode.FILE, false);
                    System.out.println("RELOAD_SWITCH=ACTIVE_BEGIN");
                } catch (ShouldNeverHappenException expected) {
                    if (code != 16 || !expected.getMessage().contains("CommitRetryTimeout")) {
                        throw expected;
                    }
                    System.out.println("RELOAD_SWITCH=UNHANDLED_16");
                }
            }
        } finally {
            manager.destroy();
        }
    }
}
