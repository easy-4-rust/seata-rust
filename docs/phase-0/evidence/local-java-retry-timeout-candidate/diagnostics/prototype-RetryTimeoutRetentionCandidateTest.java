/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements. See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0.
 * You may obtain a copy at http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an AS IS BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 */
package org.apache.seata.server.session;

import org.apache.seata.common.ConfigurationKeys;
import org.apache.seata.common.exception.ShouldNeverHappenException;
import org.apache.seata.common.store.SessionMode;
import org.apache.seata.config.Configuration;
import org.apache.seata.config.ConfigurationFactory;
import org.apache.seata.core.exception.TransactionException;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.core.model.LockStatus;
import org.apache.seata.server.storage.file.session.FileSessionManager;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;
import org.junit.jupiter.params.provider.EnumSource;
import org.mockito.MockedStatic;

import java.lang.reflect.Method;
import java.util.Collections;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.doThrow;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.mockStatic;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

/** 以真实reload入口验证超时保留及原解锁配置；不启动或替换TC调度器。 */
class RetryTimeoutRetentionCandidateTest {
    @BeforeAll
    static void loadOriginalConfigurationBeforeScopedMocks() {
        SessionHolder.getRootSessionManager();
    }

    @ParameterizedTest
    @CsvSource({
        "FILE,true,false,false", "FILE,false,false,false",
        "RAFT,true,false,false", "RAFT,false,false,false",
        "FILE,true,true,false", "FILE,false,true,false",
        "RAFT,true,true,false", "RAFT,false,true,false",
        "FILE,true,false,true", "FILE,false,false,true",
        "RAFT,true,false,true", "RAFT,false,false,true",
        "FILE,true,true,true", "FILE,false,true,true",
        "RAFT,true,true,true", "RAFT,false,true,true"
    })
    void rollbackTimeoutRetainsIdentityAndHonorsBothUnlockPolicies(
            SessionMode mode, boolean acquireLock, boolean timeoutUnlock, boolean failedUnlock) throws Exception {
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
            assertDoesNotThrow(() -> SessionHolder.reload(Collections.singletonList(session), mode, acquireLock));
        }
        boolean unlock = timeoutUnlock || failedUnlock;
        verify(session).setActive(false);
        verify(session, never()).changeGlobalStatus(any());
        verify(session, never()).end();
        verify(session, times(mode == SessionMode.RAFT && unlock ? 1 : 0)).clean();
        verify(branch, times(acquireLock && !unlock ? 1 : 0)).lock();
        verify(branch, times(acquireLock && !unlock ? 1 : 0)).setLockStatus(LockStatus.Rollbacking);
    }

    @ParameterizedTest
    @CsvSource({"FILE,true", "FILE,false", "RAFT,true", "RAFT,false"})
    void commitTimeoutRemainsOperatorControlledWithoutReacquiringCommitLocks(SessionMode mode, boolean acquireLock)
            throws Exception {
        GlobalSession session = mock(GlobalSession.class);
        BranchSession branch = mock(BranchSession.class);
        when(session.getStatus()).thenReturn(GlobalStatus.CommitRetryTimeout);
        when(session.getSortedBranches()).thenReturn(Collections.singletonList(branch));
        assertDoesNotThrow(() -> SessionHolder.reload(Collections.singletonList(session), mode, acquireLock));
        verify(session).setActive(false);
        verify(session, never()).changeGlobalStatus(any());
        verify(session, never()).end();
        verify(session, times(mode == SessionMode.RAFT ? 1 : 0)).clean();
        verify(branch, never()).lock();
    }

    @Test
    void failedLockAcquisitionIsNotReportedAsSafeRestore() throws Exception {
        GlobalSession session = rollbackSessionWithBranch(false);
        ShouldNeverHappenException error = assertThrows(
                ShouldNeverHappenException.class,
                () -> SessionHolder.reload(Collections.singletonList(session), SessionMode.FILE, true));
        assertTrue(error.getMessage().contains("Could not restore retry-timeout branch lock"), error.toString());
    }

    @Test
    void lockStorageExceptionRemainsVisible() throws Exception {
        GlobalSession session = rollbackSessionWithBranch(true);
        BranchSession branch = session.getSortedBranches().get(0);
        doThrow(new TransactionException("owned restore lock failure"))
                .when(branch)
                .lock();
        ShouldNeverHappenException error = assertThrows(
                ShouldNeverHappenException.class,
                () -> SessionHolder.reload(Collections.singletonList(session), SessionMode.FILE, true));
        assertTrue(error.getCause() instanceof TransactionException);
    }

    @Test
    void raftCommitLockCleanupFailureRemainsVisible() throws Exception {
        GlobalSession session = mock(GlobalSession.class);
        when(session.getStatus()).thenReturn(GlobalStatus.CommitRetryTimeout);
        doThrow(new TransactionException("owned commit cleanup failure"))
                .when(session)
                .clean();
        RuntimeException error = assertThrows(
                RuntimeException.class,
                () -> SessionHolder.reload(Collections.singletonList(session), SessionMode.RAFT, false));
        assertTrue(error.getCause() instanceof TransactionException);
    }

    @ParameterizedTest
    @EnumSource(
            value = GlobalStatus.class,
            names = {
                "CommitRetryTimeout",
                "RollbackRetryTimeout",
                "Begin",
                "StopCommitOrCommitRetry",
                "StopRollbackOrRollbackRetry",
                "Deleting"
            })
    void fileRetainsOperatorOrActiveRecords(GlobalStatus status) throws Exception {
        assertTrue(fileRetains(status));
    }

    @ParameterizedTest
    @EnumSource(
            value = GlobalStatus.class,
            names = {
                "UnKnown",
                "Committed",
                "CommitFailed",
                "Rollbacked",
                "RollbackFailed",
                "TimeoutRollbacked",
                "TimeoutRollbackFailed",
                "Finished"
            })
    void originalCompletedAndDiscardedStatesRemainFiltered(GlobalStatus status) throws Exception {
        assertFalse(fileRetains(status));
    }

    private GlobalSession rollbackSessionWithBranch(boolean locked) throws Exception {
        GlobalSession session = mock(GlobalSession.class);
        BranchSession branch = mock(BranchSession.class);
        when(session.getStatus()).thenReturn(GlobalStatus.RollbackRetryTimeout);
        when(session.getSortedBranches()).thenReturn(Collections.singletonList(branch));
        when(session.getBranchSessions()).thenReturn(Collections.singletonList(branch));
        when(branch.lock()).thenReturn(locked);
        return session;
    }

    private boolean fileRetains(GlobalStatus status) throws Exception {
        FileSessionManager manager = new FileSessionManager("owned-retention-filter");
        Method predicate = FileSessionManager.class.getDeclaredMethod("checkSessionStatus", GlobalSession.class);
        predicate.setAccessible(true);
        GlobalSession session = mock(GlobalSession.class);
        when(session.getStatus()).thenReturn(status);
        return (Boolean) predicate.invoke(manager, session);
    }
}
