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
package org.apache.seata.core.rpc.netty.v1;

import com.fasterxml.jackson.databind.ObjectMapper;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.ByteBufUtil;
import io.netty.buffer.Unpooled;
import org.apache.seata.core.exception.TransactionExceptionCode;
import org.apache.seata.core.model.GlobalStatus;
import org.apache.seata.core.protocol.AbstractMessage;
import org.apache.seata.core.protocol.AbstractResultMessage;
import org.apache.seata.core.protocol.HeartbeatMessage;
import org.apache.seata.core.protocol.MergeResultMessage;
import org.apache.seata.core.protocol.MergedWarpMessage;
import org.apache.seata.core.protocol.ResultCode;
import org.apache.seata.core.protocol.RpcMessage;
import org.apache.seata.core.protocol.transaction.GlobalBeginRequest;
import org.apache.seata.core.protocol.transaction.GlobalBeginResponse;
import org.apache.seata.core.protocol.transaction.GlobalCommitResponse;
import org.apache.seata.core.protocol.transaction.GlobalStatusRequest;
import org.apache.seata.serializer.seata.serializer.SeataSerializerV1;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 原 Java V1 合并消息及无消息体心跳的字段／线缆特征测试；不是生产协议实现。 */
class ProtocolV1ControlEnvelopeTest {
    private MergedWarpMessage requests(boolean ids) {
        GlobalBeginRequest begin = new GlobalBeginRequest();
        begin.setTimeout(2000);
        begin.setTransactionName("m-a");
        GlobalStatusRequest status = new GlobalStatusRequest();
        status.setXid("a");
        MergedWarpMessage message = new MergedWarpMessage();
        message.msgs.add(begin);
        message.msgs.add(status);
        if (ids) message.msgIds.addAll(Arrays.asList(7, -9));
        return message;
    }

    private AbstractMessage merge(String name) {
        if ("MergeRequestWithIds".equals(name)) return requests(true);
        if ("MergeRequestWithoutIds".equals(name)) return requests(false);
        if ("MergeRequestEmpty".equals(name)) return new MergedWarpMessage();
        if ("MergeRequestNested".equals(name)) {
            MergedWarpMessage nested = new MergedWarpMessage();
            nested.msgs.add(requests(false));
            nested.msgIds.add(42);
            return nested;
        }
        MergeResultMessage result = new MergeResultMessage();
        if ("MergeResultEmpty".equals(name)) result.setMsgs(new AbstractResultMessage[0]);
        else {
            GlobalBeginResponse begin = new GlobalBeginResponse();
            begin.setXid("a");
            begin.setResultCode(ResultCode.Success);
            begin.setTransactionExceptionCode(TransactionExceptionCode.Unknown);
            GlobalCommitResponse commit = new GlobalCommitResponse();
            commit.setGlobalStatus(GlobalStatus.Committed);
            commit.setResultCode(ResultCode.Success);
            commit.setTransactionExceptionCode(TransactionExceptionCode.Unknown);
            result.setMsgs(new AbstractResultMessage[] {begin, commit});
        }
        return result;
    }

    private void assertRequests(MergedWarpMessage message, boolean ids) {
        assertEquals(2, message.msgs.size());
        GlobalBeginRequest begin = assertInstanceOf(GlobalBeginRequest.class, message.msgs.get(0));
        assertEquals(2000, begin.getTimeout());
        assertEquals("m-a", begin.getTransactionName());
        GlobalStatusRequest status = assertInstanceOf(GlobalStatusRequest.class, message.msgs.get(1));
        assertEquals("a", status.getXid());
        assertNull(status.getExtraData());
        assertEquals(ids ? Arrays.asList(7, -9) : new ArrayList<Integer>(), message.msgIds);
    }

    @ParameterizedTest
    @ValueSource(
            strings = {
                "HeartbeatPing",
                "HeartbeatPong",
                "HeartbeatHeadMap",
                "MergeRequestWithIds",
                "MergeRequestWithoutIds",
                "MergeRequestEmpty",
                "MergeRequestNested",
                "MergeResult",
                "MergeResultEmpty"
            })
    void originalEncoderDecoderPreserveControlFieldsAndExactBytes(String name) throws Exception {
        RpcMessage input = new RpcMessage();
        input.setId(0x10203040);
        input.setCodec((byte) 1);
        input.setCompressor((byte) 0);
        boolean heartbeat = name.startsWith("Heartbeat");
        input.setMessageType(
                (byte) (heartbeat ? ("HeartbeatPong".equals(name) ? 4 : 3) : (name.startsWith("MergeResult") ? 1 : 0)));
        input.setBody(
                heartbeat
                        ? ("HeartbeatPong".equals(name) ? HeartbeatMessage.PONG : HeartbeatMessage.PING)
                        : merge(name));
        if ("HeartbeatHeadMap".equals(name)) input.getHeadMap().put("traceId", "hb");
        ByteBuf frame = Unpooled.buffer();
        ByteBuf encodedAgain = Unpooled.buffer();
        try {
            new ProtocolEncoderV1().encode(input, frame);
            byte[] expected = ByteBufUtil.getBytes(frame);
            RpcMessage decoded = new ProtocolDecoderV1().decodeFrame(frame);
            assertEquals(0, frame.readableBytes());
            assertEquals(0x10203040, decoded.getId());
            assertEquals(input.getMessageType(), decoded.getMessageType());
            assertEquals(input.getHeadMap(), decoded.getHeadMap());
            if (heartbeat)
                assertEquals(
                        !"HeartbeatPong".equals(name),
                        assertInstanceOf(HeartbeatMessage.class, decoded.getBody())
                                .isPing());
            else if ("MergeRequestWithIds".equals(name) || "MergeRequestWithoutIds".equals(name))
                assertRequests(
                        assertInstanceOf(MergedWarpMessage.class, decoded.getBody()),
                        "MergeRequestWithIds".equals(name));
            else if ("MergeRequestEmpty".equals(name))
                assertTrue(assertInstanceOf(MergedWarpMessage.class, decoded.getBody())
                        .msgs
                        .isEmpty());
            else if ("MergeRequestNested".equals(name)) {
                MergedWarpMessage outer = assertInstanceOf(MergedWarpMessage.class, decoded.getBody());
                assertEquals(Arrays.asList(42), outer.msgIds);
                assertEquals(1, outer.msgs.size());
                assertRequests(assertInstanceOf(MergedWarpMessage.class, outer.msgs.get(0)), false);
            } else {
                MergeResultMessage result = assertInstanceOf(MergeResultMessage.class, decoded.getBody());
                assertEquals("MergeResultEmpty".equals(name) ? 0 : 2, result.msgs.length);
                if (result.msgs.length > 0) {
                    GlobalBeginResponse begin = assertInstanceOf(GlobalBeginResponse.class, result.msgs[0]);
                    assertEquals("a", begin.getXid());
                    assertNull(begin.getExtraData());
                    assertEquals(
                            GlobalStatus.Committed,
                            assertInstanceOf(GlobalCommitResponse.class, result.msgs[1])
                                    .getGlobalStatus());
                }
            }
            new ProtocolEncoderV1().encode(decoded, encodedAgain);
            assertArrayEquals(expected, ByteBufUtil.getBytes(encodedAgain));
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("name", name);
            row.put("frame_hex", ByteBufUtil.hexDump(expected));
            row.put(
                    "serialized_hex",
                    heartbeat
                            ? ""
                            : ByteBufUtil.hexDump(
                                    SeataSerializerV1.getInstance().serialize(decoded.getBody())));
            row.put("kind", decoded.getMessageType());
            row.put("request_id", decoded.getId());
            System.out.println("SEATA_CONTROL_WIRE " + new ObjectMapper().writeValueAsString(row));
        } finally {
            frame.release();
            encodedAgain.release();
        }
    }
}
