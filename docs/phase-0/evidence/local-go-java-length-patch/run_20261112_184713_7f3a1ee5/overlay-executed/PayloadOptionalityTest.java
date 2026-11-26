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
package org.apache.seata.serializer.seata;

import org.apache.seata.core.protocol.AbstractIdentifyResponse;
import org.apache.seata.core.protocol.Version;
import org.apache.seata.core.protocol.transaction.GlobalReportRequest;
import org.junit.jupiter.api.Test;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;

/** 固定 Java V1 的合法缺省字段语义，与 Rust 线缆字段适配独立核对。 */
class PayloadOptionalityTest {
    @Test
    void globalReportNullStatusUsesMinusOne() {
        byte[] payload = {0, 17, 0, 0, 0, 0, (byte) 255};
        SeataSerializer serializer = new SeataSerializer((byte) 1);
        GlobalReportRequest decoded = serializer.deserialize(payload);
        assertNull(decoded.getXid());
        assertNull(decoded.getExtraData());
        assertNull(decoded.getGlobalStatus());
        assertArrayEquals(payload, serializer.serialize(decoded));
        assertArrayEquals(payload, serializer.serialize(new GlobalReportRequest()));
    }

    @Test
    void tmResponseAbsentVersionRetainsJavaBuildDefault() {
        verifyAbsentVersion((byte) 102);
    }

    @Test
    void rmResponseAbsentVersionRetainsJavaBuildDefault() {
        verifyAbsentVersion((byte) 104);
    }

    private static void verifyAbsentVersion(byte type) {
        byte[] payload = {0, type, 0, 0, 0};
        SeataSerializer serializer = new SeataSerializer((byte) 1);
        AbstractIdentifyResponse decoded = serializer.deserialize(payload);
        assertFalse(decoded.isIdentified());
        assertEquals("2.8.0-SNAPSHOT", Version.getCurrent(), "Pinned Java build changed");
        assertEquals(Version.getCurrent(), decoded.getVersion());
        byte[] version = Version.getCurrent().getBytes(StandardCharsets.UTF_8);
        byte[] javaNormalized = ByteBuffer.allocate(5 + version.length)
                .putShort((short) type)
                .put((byte) 0)
                .putShort((short) version.length)
                .put(version)
                .array();
        assertArrayEquals(javaNormalized, serializer.serialize(decoded));
        // Java 默认值会在重编码中显式写入；这与线缆原始缺省信息是不同维度。
        assertFalse(Arrays.equals(payload, javaNormalized));
    }
}
