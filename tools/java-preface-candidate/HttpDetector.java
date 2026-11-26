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
package org.apache.seata.core.protocol.detector;

import io.netty.buffer.ByteBuf;
import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.ChannelPipeline;
import io.netty.handler.codec.http.HttpObjectAggregator;
import io.netty.handler.codec.http.HttpServerCodec;
import io.netty.handler.codec.http.HttpServerUpgradeHandler;
import io.netty.handler.codec.http2.Http2CodecUtil;
import io.netty.handler.codec.http2.Http2FrameCodecBuilder;
import io.netty.handler.codec.http2.Http2MultiplexHandler;
import io.netty.handler.codec.http2.Http2ServerUpgradeCodec;
import io.netty.handler.codec.http2.Http2StreamChannel;
import io.netty.util.AsciiString;
import lombok.extern.slf4j.Slf4j;
import org.apache.seata.core.rpc.netty.http.Http2HttpHandler;
import org.apache.seata.core.rpc.netty.http.HttpDispatchHandler;

/** 保持原八字节HTTP识别规则，同时允许七种原方法的前缀分片。 */
@Slf4j
public class HttpDetector implements IncrementalProtocolDetector {
    private static final String[] HTTP_METHODS = {"GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH"};

    /** 按原八字节阈值识别HTTP方法，不消费调用方的可读区。 */
    @Override
    public boolean detect(ByteBuf in) {
        if (in.readableBytes() < 8) {
            return false;
        }

        for (String method : HTTP_METHODS) {
            if (startsWith(in, method)) {
                return true;
            }
        }

        return false;
    }

    /** 不足原识别阈值时，只有匹配至少一个原HTTP方法的前缀才继续等待。 */
    @Override
    public boolean needsMoreData(ByteBuf in) {
        if (in.readableBytes() >= 8) return false;
        for (String method : HTTP_METHODS) {
            boolean matches = true;
            for (int i = 0; i < Math.min(in.readableBytes(), method.length()); i++) {
                if (in.getByte(in.readerIndex() + i) != (byte) method.charAt(i)) {
                    matches = false;
                    break;
                }
            }
            if (matches) return true;
        }
        return false;
    }

    /** 在实际readerIndex处比较方法，不读取已消费前缀。 */
    private boolean startsWith(ByteBuf buffer, String prefix) {
        for (int i = 0; i < prefix.length(); i++) {
            if (buffer.getByte(buffer.readerIndex() + i) != (byte) prefix.charAt(i)) {
                return false;
            }
        }

        return true;
    }

    @Override
    public ChannelHandler[] getHandlers() {
        HttpServerCodec sourceCodec = new HttpServerCodec();
        HttpServerUpgradeHandler upgradeHandler = getHttpServerUpgradeHandler(sourceCodec);

        ChannelInboundHandlerAdapter upgradeCleanupHandler = new ChannelInboundHandlerAdapter() {
            @Override
            public void userEventTriggered(ChannelHandlerContext ctx, Object evt) throws Exception {
                if (evt instanceof HttpServerUpgradeHandler.UpgradeEvent) {
                    ChannelPipeline p = ctx.pipeline();
                    p.remove(HttpObjectAggregator.class);
                    p.remove(HttpDispatchHandler.class);
                }
                super.userEventTriggered(ctx, evt);
            }
        };

        ChannelInboundHandlerAdapter finalExceptionHandler = new ChannelInboundHandlerAdapter() {
            @Override
            public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) {
                if (cause instanceof java.io.IOException) {
                    log.trace("Connection closed by client: {}", cause.getMessage());
                } else {
                    log.error("Exception caught in HTTP pipeline: ", cause);
                }
                ctx.close();
            }
        };

        return new ChannelHandler[] {
            sourceCodec,
            upgradeHandler,
            upgradeCleanupHandler,
            new HttpObjectAggregator(1048576),
            new HttpDispatchHandler(),
            finalExceptionHandler
        };
    }

    private static HttpServerUpgradeHandler getHttpServerUpgradeHandler(HttpServerCodec sourceCodec) {
        HttpServerUpgradeHandler.UpgradeCodecFactory upgradeCodecFactory = protocol -> {
            if (AsciiString.contentEquals(Http2CodecUtil.HTTP_UPGRADE_PROTOCOL_NAME, protocol)) {
                return new Http2ServerUpgradeCodec(
                        Http2FrameCodecBuilder.forServer().build(),
                        new Http2MultiplexHandler(new ChannelInitializer<Http2StreamChannel>() {
                            @Override
                            protected void initChannel(Http2StreamChannel ch) {
                                ch.pipeline().addLast(new Http2HttpHandler());
                            }
                        }));
            } else {
                return null;
            }
        };

        return new HttpServerUpgradeHandler(sourceCodec, upgradeCodecFactory, 1048576);
    }
}
