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
package org.apache.seata.core.rpc.netty.v1;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import org.apache.seata.core.compressor.CompressorType;
import org.apache.seata.core.protocol.RegisterRMRequest;
import org.apache.seata.core.protocol.RegisterTMRequest;
import org.apache.seata.core.protocol.RpcMessage;
import org.apache.seata.core.protocol.transaction.GlobalBeginRequest;
import org.apache.seata.core.protocol.transaction.GlobalBeginResponse;
import org.apache.seata.core.serializer.SerializerType;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;

class ProtocolV1GoldenFrameTest {

    private static byte[] hex(String value) {
        String normalized = value.replaceAll("\\s+", "");
        byte[] result = new byte[normalized.length() / 2];
        for (int i = 0; i < result.length; i++) {
            result[i] = (byte) Integer.parseInt(normalized.substring(i * 2, i * 2 + 2), 16);
        }
        return result;
    }

    private static RpcMessage decodeAndReencode(String frameHex) {
        byte[] expected = hex(frameHex);
        ProtocolDecoderV1 decoder = new ProtocolDecoderV1();
        ByteBuf input = Unpooled.wrappedBuffer(expected);
        RpcMessage decoded;
        try {
            decoded = decoder.decodeFrame(input);
        } finally {
            input.release();
        }

        ByteBuf output = Unpooled.buffer();
        try {
            new ProtocolEncoderV1().encode(decoded, output);
            byte[] actual = new byte[output.readableBytes()];
            output.readBytes(actual);
            assertArrayEquals(expected, actual, "Java V1 decoder/encoder must preserve the complete frame");
        } finally {
            output.release();
        }
        return decoded;
    }

    @Test
    void decodesAndReencodesGoGoldenGlobalBeginRequestFrame() {
        RpcMessage message = decodeAndReencode(
                "dada010000002a0010000100000000010001000007d000125365617461476f5472616e73616374696f6e");

        assertEquals(1, message.getId());
        assertEquals(0, message.getMessageType());
        assertEquals(SerializerType.SEATA.getCode(), message.getCodec());
        assertEquals(CompressorType.NONE.getCode(), message.getCompressor());

        GlobalBeginRequest request = assertInstanceOf(GlobalBeginRequest.class, message.getBody());
        assertEquals(2000, request.getTimeout());
        assertEquals("SeataGoTransaction", request.getTransactionName());
    }

    @Test
    void decodesAndReencodesGoGoldenGlobalBeginRequestWithHeadMapFrame() {
        RpcMessage message = decodeAndReencode(
                "dada010000003e002400010000000001000774726163654964000974726163652d3030310001000007d000125365617461476f5472616e73616374696f6e");

        assertEquals(1, message.getId());
        assertEquals("trace-001", message.getHeadMap().get("traceId"));
        GlobalBeginRequest request = assertInstanceOf(GlobalBeginRequest.class, message.getBody());
        assertEquals(2000, request.getTimeout());
        assertEquals("SeataGoTransaction", request.getTransactionName());
    }

    @Test
    void decodesAndReencodesGoGoldenFailedGlobalBeginResponseFrame() {
        RpcMessage message = decodeAndReencode(
                "dada0100000026001001010000000001000200000c626567696e2064656e6965640100000000");

        GlobalBeginResponse response = assertInstanceOf(GlobalBeginResponse.class, message.getBody());
        assertEquals("begin denied", response.getMsg());
        assertNull(response.getXid(), "Java V1 decoder leaves zero-length XID unset");
        assertNull(response.getExtraData(), "Java V1 decoder leaves zero-length extraData unset");
    }

    @Test
    void decodesAndReencodesGoGoldenGlobalBeginResponseFrame() {
        RpcMessage message = decodeAndReencode(
                "dada01000000280010010100000000010002010000103132372e302e302e313a383039313a310000");

        assertEquals(1, message.getId());
        assertEquals(1, message.getMessageType());
        assertEquals(SerializerType.SEATA.getCode(), message.getCodec());
        assertEquals(CompressorType.NONE.getCode(), message.getCompressor());

        GlobalBeginResponse response = assertInstanceOf(GlobalBeginResponse.class, message.getBody());
        assertEquals("127.0.0.1:8091:1", response.getXid());
        assertNull(response.getExtraData(), "Java V1 decoder leaves zero-length extraData unset");
    }
    @Test
    void decodesAndReencodesGoGoldenRegisterTMRequestFrame() {
        RpcMessage message = decodeAndReencode(
                "dada01000000390010000100000000020065000456312c30000b6f72646572732d72757374001064656661756c745f74785f67726f75700000");

        RegisterTMRequest request = assertInstanceOf(RegisterTMRequest.class, message.getBody());
        assertEquals("V1,0", request.getVersion());
        assertEquals("orders-rust", request.getApplicationId());
        assertEquals("default_tx_group", request.getTransactionServiceGroup());
        assertEquals("", request.getExtraData());
    }

    @Test
    void decodesAndReencodesGoGoldenRegisterRMRequestFrame() {
        RpcMessage message = decodeAndReencode(
                "dada010000004b0010000100000000030067000456312c30000b6f72646572732d72757374001064656661756c745f74785f67726f757000000000000e6d7973716c3a2f2f6f7264657273");

        RegisterRMRequest request = assertInstanceOf(RegisterRMRequest.class, message.getBody());
        assertEquals("V1,0", request.getVersion());
        assertEquals("orders-rust", request.getApplicationId());
        assertEquals("default_tx_group", request.getTransactionServiceGroup());
        assertNull(request.getExtraData(), "Java RM decoder leaves zero-length extraData unset");
        assertEquals("mysql://orders", request.getResourceIds());
    }

}
