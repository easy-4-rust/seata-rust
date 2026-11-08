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
import io.netty.buffer.ByteBufUtil;
import io.netty.buffer.Unpooled;
import org.apache.seata.core.protocol.RpcMessage;
import org.apache.seata.core.protocol.transaction.GlobalBeginRequest;
import org.apache.seata.core.protocol.transaction.GlobalBeginResponse;
import org.apache.seata.core.serializer.SerializerType;
import org.apache.seata.core.compressor.CompressorType;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;

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
        RpcMessage decoded = decoder.decodeFrame(Unpooled.wrappedBuffer(expected));

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
    void decodesAndReencodesGoGoldenGlobalBeginResponseFrame() {
        RpcMessage message = decodeAndReencode(
                "dada01000000280010010100000000010002010000103132372e302e302e313a383039313a310000");

        assertEquals(1, message.getId());
        assertEquals(1, message.getMessageType());
        assertEquals(SerializerType.SEATA.getCode(), message.getCodec());
        assertEquals(CompressorType.NONE.getCode(), message.getCompressor());

        GlobalBeginResponse response = assertInstanceOf(GlobalBeginResponse.class, message.getBody());
        assertEquals("127.0.0.1:8091:1", response.getXid());
        assertEquals("", response.getExtraData());
    }
}
