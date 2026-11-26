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

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.seata.core.exception.TransactionExceptionCode;
import org.apache.seata.core.model.BranchStatus;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.core.protocol.ResultCode;
import org.apache.seata.core.protocol.transaction.AbstractTransactionResponse;
import org.apache.seata.core.protocol.transaction.BranchRegisterResponse;
import org.apache.seata.core.protocol.transaction.BranchReportResponse;
import org.apache.seata.core.protocol.transaction.BranchRollbackResponse;
import org.apache.seata.core.protocol.transaction.GlobalLockQueryResponse;
import org.apache.seata.core.protocol.transaction.GlobalReportResponse;
import org.apache.seata.core.protocol.transaction.GlobalRollbackResponse;
import org.apache.seata.core.protocol.transaction.GlobalStatusResponse;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.TestFactory;

import java.io.InputStream;
import java.nio.BufferUnderflowException;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 失败变体的 Java 权威字节、字段和固定 Go 差异诊断；差异检出不等于互操作通过。 */
class FailureVariantCompatibilityTest {
    private static final ObjectMapper JSON = new ObjectMapper();

    @TestFactory
    List<DynamicTest> failureVariantsAndKnownDivergences() throws Exception {
        JsonNode rows;
        try (InputStream input = getClass().getResourceAsStream("/seata-rust-failure-variants.json")) {
            assertNotNull(input);
            rows = JSON.readTree(input);
        }
        assertTrue(rows.isArray());
        assertEquals(7, rows.size());
        Set<String> names = new HashSet<>();
        List<DynamicTest> tests = new ArrayList<>();
        for (JsonNode row : rows) {
            String name = row.path("name").asText();
            assertTrue(names.add(name));
            AbstractTransactionResponse expected = expected(name);
            tests.add(DynamicTest.dynamicTest(name, () -> verify(row, expected)));
        }
        return tests;
    }

    private static void verify(JsonNode row, AbstractTransactionResponse expected) {
        String name = row.path("name").asText();
        SeataSerializer serializer = new SeataSerializer((byte) 1);
        byte[] javaBytes = hex(row.path("java_serialized_hex").asText());
        byte[] goBytes = hex(row.path("go_serialized_hex").asText());
        assertEquals(row.path("type_id").asInt(), expected.getTypeCode());
        assertArrayEquals(javaBytes, serializer.serialize(expected), "Independent typed Java encoder");
        AbstractTransactionResponse decoded = serializer.deserialize(javaBytes);
        assertEquals(JSON.valueToTree(expected), JSON.valueToTree(decoded), "Independent field semantics");
        assertArrayEquals(javaBytes, serializer.serialize(decoded));
        assertEquals(ResultCode.Failed, decoded.getResultCode());
        if (name.equals("BranchReportResponseFailed") || name.equals("GlobalLockQueryResponseFailed")) {
            assertEquals("error_text_length", row.path("compatibility").asText());
            assertNotEquals(row.path("java_serialized_hex").asText(), row.path("go_serialized_hex").asText());
            assertThrows(BufferUnderflowException.class, () -> serializer.deserialize(goBytes));
        } else if (name.equals("BranchRegisterLockKeyConflictFailFast")) {
            assertEquals("exception_code_ordinal", row.path("compatibility").asText());
            BranchRegisterResponse misinterpreted = serializer.deserialize(goBytes);
            assertEquals(ResultCode.Failed, misinterpreted.getResultCode());
            assertEquals("fail fast", misinterpreted.getMsg());
            assertEquals(TransactionExceptionCode.NotRaftLeader, misinterpreted.getTransactionExceptionCode());
            assertNotEquals(expected.getTransactionExceptionCode(), misinterpreted.getTransactionExceptionCode());
        } else {
            assertEquals("byte_equal", row.path("compatibility").asText());
            assertArrayEquals(javaBytes, goBytes);
            assertEquals(JSON.valueToTree(expected), JSON.valueToTree(serializer.deserialize(goBytes)));
        }
    }

    private static AbstractTransactionResponse expected(String name) {
        switch (name) {
            case "BranchRollbackResponseFailed":
                BranchRollbackResponse branch = failed(new BranchRollbackResponse(), TransactionExceptionCode.BranchRollbackFailed_Retriable, "rollback retry");
                branch.setXid("127.0.0.1:8091:1");
                branch.setBranchId(0x0102030405060708L);
                branch.setBranchStatus(BranchStatus.PhaseTwo_RollbackFailed_Retryable);
                return branch;
            case "GlobalRollbackResponseFailed":
                GlobalRollbackResponse rollback = failed(new GlobalRollbackResponse(), TransactionExceptionCode.IO, "rollback pending");
                rollback.setGlobalStatus(GlobalStatus.RollbackRetrying);
                return rollback;
            case "BranchReportResponseFailed":
                return failed(new BranchReportResponse(), TransactionExceptionCode.BranchReportFailed, "report denied");
            case "GlobalStatusResponseFailed":
                GlobalStatusResponse status = failed(new GlobalStatusResponse(), TransactionExceptionCode.IO, "status unavailable");
                status.setGlobalStatus(GlobalStatus.UnKnown);
                return status;
            case "GlobalReportResponseFailed":
                GlobalReportResponse report = failed(new GlobalReportResponse(), TransactionExceptionCode.IO, "report pending");
                report.setGlobalStatus(GlobalStatus.Rollbacking);
                return report;
            case "GlobalLockQueryResponseFailed":
                GlobalLockQueryResponse lock = failed(new GlobalLockQueryResponse(), TransactionExceptionCode.LockableCheckFailed, "lock check failed");
                lock.setLockable(false);
                return lock;
            case "BranchRegisterLockKeyConflictFailFast":
                BranchRegisterResponse register = failed(new BranchRegisterResponse(), TransactionExceptionCode.LockKeyConflictFailFast, "fail fast");
                register.setBranchId(0L);
                return register;
            default: throw new IllegalArgumentException("Unknown failure case " + name);
        }
    }

    private static <T extends AbstractTransactionResponse> T failed(T response, TransactionExceptionCode code, String msg) {
        response.setResultCode(ResultCode.Failed);
        response.setTransactionExceptionCode(code);
        response.setMsg(msg);
        return response;
    }

    private static byte[] hex(String value) {
        assertTrue(value.matches("(?:[0-9a-f]{2})+"));
        byte[] bytes = new byte[value.length() / 2];
        for (int at = 0; at < bytes.length; at++) {
            bytes[at] = (byte) Integer.parseInt(value.substring(at * 2, at * 2 + 2), 16);
        }
        return bytes;
    }
}
