/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements.  See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License.  You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
package org.apache.seata.server.session;

import org.apache.seata.common.exception.ShouldNeverHappenException;
import org.apache.seata.common.store.SessionMode;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.server.storage.file.session.FileSessionManager;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Method;
import java.util.Collections;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

class SessionHolderRetryTimeoutRecoveryTest {

    @Test
    void fileRestoreFiltersRollbackRetryTimeoutButRetainsCommitRetryTimeout() throws Exception {
        FileSessionManager manager = new FileSessionManager("retry-timeout-test");
        Method checkSessionStatus =
                FileSessionManager.class.getDeclaredMethod("checkSessionStatus", GlobalSession.class);
        checkSessionStatus.setAccessible(true);

        GlobalSession rollbackRetryTimeout = mock(GlobalSession.class);
        when(rollbackRetryTimeout.getStatus()).thenReturn(GlobalStatus.RollbackRetryTimeout);

        GlobalSession commitRetryTimeout = mock(GlobalSession.class);
        when(commitRetryTimeout.getStatus()).thenReturn(GlobalStatus.CommitRetryTimeout);

        assertFalse((Boolean) checkSessionStatus.invoke(manager, rollbackRetryTimeout));
        assertTrue((Boolean) checkSessionStatus.invoke(manager, commitRetryTimeout));
    }

    @Test
    void fileSessionHolderReloadRejectsCommitRetryTimeoutAfterFilePrefilterRetainsIt() {
        GlobalSession session = mock(GlobalSession.class);
        when(session.getStatus()).thenReturn(GlobalStatus.CommitRetryTimeout);

        assertThrows(
                ShouldNeverHappenException.class,
                () -> SessionHolder.reload(Collections.singletonList(session), SessionMode.FILE, false));
    }

    @Test
    void raftLeaderReloadHasNoExplicitCaseForEitherRetryTimeoutState() {
        for (GlobalStatus status : new GlobalStatus[] {
            GlobalStatus.CommitRetryTimeout, GlobalStatus.RollbackRetryTimeout
        }) {
            GlobalSession session = mock(GlobalSession.class);
            when(session.getStatus()).thenReturn(status);

            assertThrows(
                    ShouldNeverHappenException.class,
                    () -> SessionHolder.reload(Collections.singletonList(session), SessionMode.RAFT, false),
                    "unexpected status must be explicit for " + status);
        }
    }
}
