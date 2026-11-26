/* Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0. */
package org.apache.seata.serializer.seata;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.seata.core.exception.TransactionExceptionCode;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.core.protocol.AbstractIdentifyResponse;
import org.apache.seata.core.protocol.AbstractMessage;
import org.apache.seata.core.protocol.AbstractResultMessage;
import org.apache.seata.core.protocol.MergeResultMessage;
import org.apache.seata.core.protocol.MergedWarpMessage;
import org.apache.seata.core.protocol.RegisterRMResponse;
import org.apache.seata.core.protocol.RegisterTMResponse;
import org.apache.seata.core.protocol.ResultCode;
import org.apache.seata.core.protocol.Version;
import org.apache.seata.core.protocol.transaction.AbstractTransactionResponse;
import org.apache.seata.core.protocol.transaction.GlobalBeginRequest;
import org.apache.seata.core.protocol.transaction.GlobalReportRequest;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.TestFactory;

import java.io.InputStream;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

/** 全消息与异常语义的合并对照，对应Java：MergedWarpMessage/MergeResultMessage及其V1 codec。 */
class MergedMessageCompatibilityTest {
    private static final ObjectMapper JSON = new ObjectMapper();

    @TestFactory
    List<DynamicTest> allMessageFamiliesAndExceptionResults() throws Exception {
        SeataSerializer serializer = new SeataSerializer((byte) 1);
        List<AbstractMessage> messages = new ArrayList<>();
        Set<Integer> types = new HashSet<>();
        try (InputStream input = getClass().getResourceAsStream("/seata-rust-payload-vectors.json")) {
            assertNotNull(input);
            for (JsonNode row : JSON.readTree(input)) {
                int type = row.path("type_id").asInt();
                if (types.add(type))
                    messages.add(serializer.deserialize(
                            hex(row.path("serialized_hex").asText())));
            }
        }
        assertEquals(25, messages.size());
        List<DynamicTest> tests = new ArrayList<>();
        for (AbstractMessage message : messages) {
            MergedWarpMessage single = new MergedWarpMessage();
            single.msgs.add(message);
            single.msgIds.add(-100 - (int) message.getTypeCode());
            tests.add(test("Single_" + message.getTypeCode(), single));
        }
        MergedWarpMessage all = new MergedWarpMessage();
        all.msgs.addAll(messages);
        for (int i = 0; i < messages.size(); i++) all.msgIds.add(i % 2 == 0 ? i + 1 : -i - 1);
        tests.add(test("All25WithIds", all));
        MergedWarpMessage without = new MergedWarpMessage();
        without.msgs.addAll(messages);
        tests.add(test("All25WithoutIds", without));
        List<AbstractResultMessage> results = new ArrayList<>();
        for (AbstractMessage message : messages)
            if (message instanceof AbstractResultMessage) results.add((AbstractResultMessage) message);
        assertEquals(12, results.size());
        MergeResultMessage mergedResults = new MergeResultMessage();
        mergedResults.setMsgs(results.toArray(new AbstractResultMessage[0]));
        tests.add(test("All12Results", mergedResults));
        List<AbstractResultMessage> exceptions = new ArrayList<>();
        for (int type : new int[] {2, 4, 6, 8, 10, 12, 14, 16, 18, 22}) {
            for (TransactionExceptionCode code : TransactionExceptionCode.values()) {
                AbstractTransactionResponse response = TransactionExceptionProfileTest.response(type);
                response.setResultCode(ResultCode.Failed);
                response.setTransactionExceptionCode(code);
                response.setMsg("wire:" + code.name());
                exceptions.add(response);
            }
        }
        assertEquals(250, exceptions.size());
        MergeResultMessage allErrors = new MergeResultMessage();
        allErrors.setMsgs(exceptions.toArray(new AbstractResultMessage[0]));
        tests.add(test("All250ExceptionResults", allErrors));
        MergedWarpMessage nested = new MergedWarpMessage();
        nested.msgs.add(all);
        nested.msgs.add(mergedResults);
        nested.msgIds.add(77);
        nested.msgIds.add(-77);
        tests.add(test("NestedAll", nested));
        for (int timeout : new int[] {-1, Integer.MIN_VALUE, 0, Integer.MAX_VALUE}) {
            GlobalBeginRequest request = new GlobalBeginRequest();
            request.setTimeout(timeout);
            request.setTransactionName("signed");
            MergedWarpMessage one = new MergedWarpMessage();
            one.msgs.add(request);
            tests.add(test("SignedTimeout_" + timeout, one));
        }
        tests.add(absentVersion("AbsentTMVersion", new RegisterTMResponse()));
        tests.add(absentVersion("AbsentRMVersion", new RegisterRMResponse()));
        GlobalReportRequest absentStatus = new GlobalReportRequest();
        absentStatus.setXid("ctx");
        absentStatus.setExtraData("meta");
        absentStatus.setGlobalStatus(null);
        MergedWarpMessage absent = new MergedWarpMessage();
        absent.msgs.add(absentStatus);
        tests.add(test("NullReportStatus", absent));
        GlobalReportRequest unknownStatus = new GlobalReportRequest();
        unknownStatus.setXid("ctx");
        unknownStatus.setExtraData("meta");
        unknownStatus.setGlobalStatus(GlobalStatus.UnKnown);
        MergedWarpMessage unknown = new MergedWarpMessage();
        unknown.msgs.add(unknownStatus);
        tests.add(test("UnknownReportStatus", unknown));
        assertEquals(38, tests.size());
        return tests;
    }

    private static DynamicTest absentVersion(String name, AbstractIdentifyResponse response) {
        return DynamicTest.dynamicTest(name, () -> {
            response.setVersion(null);
            response.setIdentified(false);
            MergedWarpMessage original = new MergedWarpMessage();
            original.msgs.add(response);
            SeataSerializer serializer = new SeataSerializer((byte) 1);
            byte[] wire = serializer.serialize(original);
            MergedWarpMessage decoded = serializer.deserialize(wire);
            AbstractIdentifyResponse item = (AbstractIdentifyResponse) decoded.msgs.get(0);
            assertEquals("2.8.0-SNAPSHOT", Version.getCurrent());
            assertEquals(Version.getCurrent(), item.getVersion());
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("name", name);
            row.put("serialized_hex", hex(wire));
            row.put("java_effective_version", item.getVersion());
            row.put("java_reencoded_hex", hex(serializer.serialize(decoded)));
            System.out.println("SEATA_MERGE_PROFILE " + JSON.writeValueAsString(row));
        });
    }

    private static DynamicTest test(String name, AbstractMessage value) {
        return DynamicTest.dynamicTest(name, () -> {
            SeataSerializer serializer = new SeataSerializer((byte) 1);
            byte[] wire = serializer.serialize(value);
            AbstractMessage decoded = serializer.deserialize(wire);
            assertEquals(JSON.valueToTree(value), JSON.valueToTree(decoded));
            assertArrayEquals(wire, serializer.serialize(decoded));
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("name", name);
            row.put("serialized_hex", hex(wire));
            System.out.println("SEATA_MERGE_PROFILE " + JSON.writeValueAsString(row));
        });
    }

    private static byte[] hex(String text) {
        byte[] out = new byte[text.length() / 2];
        for (int i = 0; i < out.length; i++) out[i] = (byte) Integer.parseInt(text.substring(i * 2, i * 2 + 2), 16);
        return out;
    }

    private static String hex(byte[] bytes) {
        char[] digits = "0123456789abcdef".toCharArray();
        char[] out = new char[bytes.length * 2];
        for (int i = 0; i < bytes.length; i++) {
            int value = bytes[i] & 0xff;
            out[i * 2] = digits[value >>> 4];
            out[i * 2 + 1] = digits[value & 15];
        }
        return new String(out);
    }
}
