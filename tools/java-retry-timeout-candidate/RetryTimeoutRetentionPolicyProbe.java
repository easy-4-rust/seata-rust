/* Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements. See NOTICE. Licensed under Apache-2.0:
 * http://www.apache.org/licenses/LICENSE-2.0 . Provided on an AS IS BASIS. */
package org.apache.seata.server.session;

import org.apache.seata.common.ConfigurationKeys;
import org.apache.seata.common.store.SessionMode;
import org.apache.seata.config.Configuration;
import org.apache.seata.config.ConfigurationFactory;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.core.model.LockStatus;
import org.mockito.MockedStatic;

import java.util.Collections;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.mockStatic;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

/** 独立JVM先冻结协调器配置，再改变provider返回值，避免静态mock污染其他测试。 */
public final class RetryTimeoutRetentionPolicyProbe {
    private RetryTimeoutRetentionPolicyProbe() {}

    /** 参数为存储模式、恢复锁标记和原两个解锁开关；退出0表示实际reload仍遵守启动快照。 */
    public static void main(String[] args) throws Exception {
        if (args.length != 4) {
            throw new IllegalArgumentException(
                    "Expected mode, acquireLock, startup timeoutUnlock, startup failedUnlock");
        }
        SessionMode mode = SessionMode.valueOf(args[0]);
        boolean acquireLock = Boolean.parseBoolean(args[1]);
        boolean timeoutUnlock = Boolean.parseBoolean(args[2]);
        boolean failedUnlock = Boolean.parseBoolean(args[3]);
        boolean startupUnlock = timeoutUnlock || failedUnlock;
        GlobalSession session = mock(GlobalSession.class);
        BranchSession branch = mock(BranchSession.class);
        when(session.getStatus()).thenReturn(GlobalStatus.RollbackRetryTimeout);
        when(session.getSortedBranches()).thenReturn(Collections.singletonList(branch));
        when(session.getBranchSessions()).thenReturn(Collections.singletonList(branch));
        when(branch.lock()).thenReturn(true);
        Configuration config = mock(Configuration.class);
        when(config.getBoolean(ConfigurationKeys.ROLLBACK_RETRY_TIMEOUT_UNLOCK_ENABLE, false))
                .thenReturn(timeoutUnlock);
        when(config.getBoolean(ConfigurationKeys.ROLLBACK_FAILED_UNLOCK_ENABLE, false))
                .thenReturn(failedUnlock);
        try (MockedStatic<ConfigurationFactory> factory = mockStatic(ConfigurationFactory.class)) {
            factory.when(ConfigurationFactory::getInstance).thenReturn(config);
            // 不引用候选新增API，保证同一测试代码能在原版及候选上编译执行。
            Class.forName("org.apache.seata.server.coordinator.DefaultCoordinator");
            Class.forName("org.apache.seata.server.session.SessionHolder");
            when(config.getBoolean(ConfigurationKeys.ROLLBACK_RETRY_TIMEOUT_UNLOCK_ENABLE, false))
                    .thenReturn(!startupUnlock);
            when(config.getBoolean(ConfigurationKeys.ROLLBACK_FAILED_UNLOCK_ENABLE, false))
                    .thenReturn(!startupUnlock);
            assertDoesNotThrow(() -> SessionHolder.reload(Collections.singletonList(session), mode, acquireLock));
            verify(session).setActive(false);
            verify(session, never()).setActive(true);
            verify(session, never()).changeGlobalStatus(any());
            verify(session, never()).queueToRetryRollback();
            verify(session, never()).queueToRetryCommit();
            verify(session, never()).end();
            verify(session, times(mode == SessionMode.RAFT && startupUnlock ? 1 : 0))
                    .clean();
            verify(branch, times(acquireLock && !startupUnlock ? 1 : 0)).lock();
            verify(branch, times(acquireLock && !startupUnlock ? 1 : 0)).setLockStatus(LockStatus.Rollbacking);
        }
        System.out.println("POLICY_SNAPSHOT_PASS mode=" + mode + " acquireLock=" + acquireLock
                + " startupTimeoutUnlock=" + timeoutUnlock + " startupFailedUnlock=" + failedUnlock);
    }
}
