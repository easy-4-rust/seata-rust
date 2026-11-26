/*
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements. See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
package org.apache.seata.serializer.seata;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.commons.lang3.StringUtils;
import org.apache.seata.core.protocol.AbstractMessage;
import org.apache.seata.core.protocol.transaction.AbstractTransactionResponse;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.TestFactory;

import java.io.InputStream;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 固定 Go 语料与 Java V1 的真实字节和对象语义交叉验证，不是 Rust 实现。 */
class PayloadGoldenCorpusTest {
    private static final ObjectMapper JSON = new ObjectMapper();
    private static final String XID = "127.0.0.1:8091:1";
    private static final long BRANCH_ID = 0x0102030405060708L;

    @TestFactory
    List<DynamicTest> allGoPayloadsDecodeAndReencodeWithJavaSemantics() throws Exception {
        JsonNode rows;
        try (InputStream input = getClass().getResourceAsStream("/seata-rust-payload-vectors.json")) {
            assertNotNull(input, "Canonical Go payload resource must be supplied by the harness");
            rows = JSON.readTree(input);
        }
        assertTrue(rows.isArray());
        assertEquals(29, rows.size(), "The complete frozen corpus must execute, without filtering failures");
        List<DynamicTest> tests = new ArrayList<>();
        Set<String> names = new HashSet<>();
        for (JsonNode row : rows) {
            String name = row.path("name").asText();
            assertTrue(StringUtils.isNotBlank(name) && names.add(name), "Missing or duplicate sample name");
            tests.add(DynamicTest.dynamicTest(name, () -> verify(row)));
        }
        return tests;
    }

    private static void verify(JsonNode row) {
        String name = row.path("name").asText();
        int type = row.path("type_id").asInt(-1);
        byte[] payload = hex(row.path("serialized_hex").asText());
        byte[] body = hex(row.path("body_hex").asText());
        assertTrue(type > 0 && type <= 65535 && payload.length >= 2);
        assertEquals(type, ((payload[0] & 255) << 8) | (payload[1] & 255));
        assertArrayEquals(body, Arrays.copyOfRange(payload, 2, payload.length));
        SeataSerializer serializer = new SeataSerializer((byte) 1);
        AbstractMessage message = serializer.deserialize(payload);
        assertNotNull(message, name);
        assertEquals(type, message.getTypeCode(), name);
        assertEquals(name.replaceFirst("Failed$", ""), message.getClass().getSimpleName());
        verifySemantics(name, type, message);
        assertArrayEquals(payload, serializer.serialize(message), name + " must preserve complete Go payload bytes");
    }

    private static void verifySemantics(String name, int type, AbstractMessage message) {
        JsonNode fields = JSON.valueToTree(message);
        boolean failed = name.endsWith("Failed");
        if (message instanceof AbstractTransactionResponse) {
            AbstractTransactionResponse response = (AbstractTransactionResponse) message;
            assertEquals(failed ? "Failed" : "Success", response.getResultCode().name());
            int error = !failed ? 0 : type == 2 ? 1 : type == 12 ? 2 : 3;
            assertEquals(error, response.getTransactionExceptionCode().ordinal());
            if (failed) {
                String text = type == 2
                        ? "begin denied"
                        : type == 8 ? "commit pending" : type == 12 ? "lock conflict" : "branch retry";
                assertEquals(text, response.getMsg());
            }
        }
        if (Arrays.asList(2, 3, 4, 5, 6, 7, 9, 11, 13, 15, 17, 21).contains(type)) {
            field(fields, "xid", failed && type == 2 ? null : XID);
        }
        if (Arrays.asList(3, 4, 5, 6, 12, 13).contains(type)) {
            field(fields, "branchId", failed && type == 12 ? 0L : BRANCH_ID);
        }
        if (Arrays.asList(3, 5, 11, 13, 21, 111).contains(type)) {
            field(fields, "resourceId", "mysql://orders");
            field(fields, "branchType", "AT");
        }
        if (Arrays.asList(3, 5, 11, 21).contains(type)) {
            field(fields, "applicationData", null);
        }
        if (Arrays.asList(2, 7, 9, 15, 17, 103).contains(type)) {
            field(fields, "extraData", null);
        }
        // TM 的共用身份解码器对零长度仍调用 setter；RM 覆盖解码器则保留 null。
        if (type == 101) {
            field(fields, "extraData", "");
        }
        switch (type) {
            case 1:
                field(fields, "timeout", 2000);
                field(fields, "transactionName", "SeataGoTransaction");
                break;
            case 4:
                field(fields, "branchStatus", failed ? "PhaseTwo_CommitFailed_Retryable" : "PhaseTwo_Committed");
                break;
            case 6:
                field(fields, "branchStatus", "PhaseTwo_Rollbacked");
                break;
            case 8:
                field(fields, "globalStatus", failed ? "Committing" : "Committed");
                break;
            case 10:
                field(fields, "globalStatus", "Rollbacked");
                break;
            case 11:
            case 21:
                field(fields, "lockKey", "orders:42");
                break;
            case 13:
                field(fields, "status", "PhaseOne_Done");
                field(fields, "applicationData", "trace=abc");
                break;
            case 16:
                field(fields, "globalStatus", "Committed");
                break;
            case 17:
            case 18:
                field(fields, "globalStatus", "Begin");
                break;
            case 22:
                field(fields, "lockable", true);
                break;
            case 101:
            case 103:
                field(fields, "version", "V1,0");
                field(fields, "applicationId", "orders-rust");
                field(fields, "transactionServiceGroup", "default_tx_group");
                if (type == 103) {
                    field(fields, "resourceIds", "mysql://orders");
                }
                break;
            case 102:
            case 104:
                field(fields, "identified", true);
                field(fields, "version", "V1,0");
                break;
            case 111:
                field(fields, "saveDays", 7);
                break;
            default:
                break;
        }
    }

    private static void field(JsonNode fields, String key, Object expected) {
        assertTrue(fields.has(key), "Java property missing: " + key);
        assertEquals(
                Objects.isNull(expected) ? JSON.getNodeFactory().nullNode() : JSON.valueToTree(expected),
                fields.get(key),
                key);
    }

    private static byte[] hex(String value) {
        assertTrue(StringUtils.isNotBlank(value) && value.length() % 2 == 0, "Malformed hex input");
        byte[] bytes = new byte[value.length() / 2];
        for (int at = 0; at < bytes.length; at++) {
            int high = Character.digit(value.charAt(at * 2), 16);
            int low = Character.digit(value.charAt(at * 2 + 1), 16);
            assertTrue(high >= 0 && low >= 0, "Non-hex input");
            bytes[at] = (byte) ((high << 4) | low);
        }
        return bytes;
    }
}
