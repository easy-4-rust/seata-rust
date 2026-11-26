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

import org.apache.seata.common.XID;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** RAFT 现场夹具的归属保护；真实选举、复制和恢复另由三节点运行验证。 */
class RaftRetryTimeoutControlTest {
    private static final String TOKEN = "phase0-raft-12345678";

    @Test
    void observerHasADistinctNumericTransactionIdForTheOriginalLockManager() {
        long transactionId = assertDoesNotThrow(() -> XID.getTransactionId(RaftRetryTimeoutControl.observerXid()));
        assertEquals(999999L, transactionId);
    }

    @Test
    void missingOwnershipMarkerIsRejectedWithoutCreatingAnything(@TempDir Path directory) {
        assertThrows(
                IllegalArgumentException.class, () -> RaftRetryTimeoutControl.validateFixture(directory, 28101, TOKEN));
        assertFalse(Files.exists(directory.resolve(".phase0-owned-raft")));
    }

    @Test
    void wrongOwnershipOutsideDrivesAndNonReservedPortsAreRejected(@TempDir Path directory) throws Exception {
        Files.writeString(directory.resolve(".phase0-owned-raft"), TOKEN);
        assertThrows(
                IllegalArgumentException.class,
                () -> RaftRetryTimeoutControl.validateFixture(directory, 28101, "wrong"));
        assertThrows(
                IllegalArgumentException.class, () -> RaftRetryTimeoutControl.validateFixture(directory, 8080, TOKEN));
        assertThrows(
                IllegalArgumentException.class,
                () -> RaftRetryTimeoutControl.validateFixture(Path.of("C:/not-owned"), 28101, TOKEN));
        assertThrows(
                IllegalArgumentException.class,
                () -> RaftRetryTimeoutControl.validateFixture(Path.of("F:/not-owned"), 28101, TOKEN));
    }

    @Test
    void ownedDirectoryAndReservedControlPortAreAccepted(@TempDir Path directory) throws Exception {
        Files.writeString(directory.resolve(".phase0-owned-raft"), TOKEN);
        assertDoesNotThrow(() -> RaftRetryTimeoutControl.validateFixture(directory, 28102, TOKEN));
    }
}
