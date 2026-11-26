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
package org.apache.seata.core.rpc.netty;

import io.netty.buffer.ByteBuf;
import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelHandlerContext;
import io.netty.handler.codec.ByteToMessageDecoder;
import lombok.extern.slf4j.Slf4j;
import org.apache.seata.core.protocol.detector.IncrementalProtocolDetector;
import org.apache.seata.core.protocol.detector.ProtocolDetector;

import java.util.List;

import static org.apache.seata.core.protocol.ProtocolConstants.MAX_FRAME_LENGTH;

/** 按实际可读区探测协议；有效但不足的前缀等待后续TCP片，不消费原输入。 */
@Slf4j
public class ProtocolDetectHandler extends ByteToMessageDecoder {
    private ProtocolDetector[] supportedProtocolDetectors;

    public ProtocolDetectHandler(ProtocolDetector[] supportedProtocolDetectors) {
        this.supportedProtocolDetectors = supportedProtocolDetectors;
    }

    /** 为每次探测创建只读独立索引视图；匹配后由Netty交付尚未消费的数据。 */
    @Override
    protected void decode(ChannelHandlerContext ctx, ByteBuf in, List<Object> out) throws Exception {
        if (in.readableBytes() > MAX_FRAME_LENGTH) {
            log.error(
                    "Packet size {} exceeds maximum {}, closing connection from {}",
                    in.readableBytes(),
                    MAX_FRAME_LENGTH,
                    ctx.channel().remoteAddress());
            ctx.close();
            return;
        }
        boolean pending = false;
        for (ProtocolDetector detector : supportedProtocolDetectors) {
            if (detector instanceof IncrementalProtocolDetector) {
                ByteBuf prefix = in.slice(in.readerIndex(), in.readableBytes()).asReadOnly();
                if (((IncrementalProtocolDetector) detector).needsMoreData(prefix)) {
                    pending = true;
                    continue;
                }
            }
            // 旧boolean扩展即使读取或标记自己的视图，也不能改变原buffer的readerIndex/mark。
            ByteBuf probe = in.slice(in.readerIndex(), in.readableBytes()).asReadOnly();
            if (detector.detect(probe)) {
                ChannelHandler[] handlers = detector.getHandlers();
                ctx.pipeline().addLast(handlers);
                ctx.pipeline().remove(this);
                return;
            }
        }
        if (pending) return;
        // 未知输入可能包含任意业务内容，仅记录长度，不输出完整或截断后的原始载荷。
        log.error(
                "Can not recognize protocol from remote {}, buffered bytes = {}",
                ctx.channel().remoteAddress(),
                in.readableBytes());
        in.clear();
        ctx.close();
    }
}
