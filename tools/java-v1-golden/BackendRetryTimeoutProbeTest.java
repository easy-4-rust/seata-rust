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

import org.apache.seata.core.rpc.netty.ChannelManager;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** 无任何存储连接前必须拒绝不属于本地测试范围的输入。 */
class BackendRetryTimeoutProbeTest {
    @Test
    void persistedOldClientAddressCanEnterTheRealChannelFallbackPath() {
        assertDoesNotThrow(() -> ChannelManager.getChannel(
                "fixture-missing-resource", BackendRetryTimeoutStoreProbe.fixtureClientId(), true));
    }

    @Test
    void absentArgumentsCannotFallBackToDefaultDatabase() {
        IllegalArgumentException error =
                assertThrows(IllegalArgumentException.class, () -> BackendRetryTimeoutStoreProbe.main(new String[0]));
        assertEquals("Expected seed/inspect and db/redis", error.getMessage());
    }

    @Test
    void unsupportedModeAndStoreAreRejectedBeforeConfigurationLoads() {
        for (String[] args : new String[][] {{"delete", "db"}, {"seed", "file"}, {"seed", "prod"}}) {
            IllegalArgumentException error =
                    assertThrows(IllegalArgumentException.class, () -> BackendRetryTimeoutStoreProbe.main(args));
            assertEquals("Unsupported backend fixture mode/store", error.getMessage());
        }
    }
}
