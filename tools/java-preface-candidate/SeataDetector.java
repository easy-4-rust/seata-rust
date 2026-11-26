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
import org.apache.seata.core.rpc.netty.MultiProtocolDecoder;

/** 识别Seata magic，支持非零可读起点与尚未完整的前缀。 */
public class SeataDetector implements IncrementalProtocolDetector {
    private static final byte[] MAGIC_CODE_BYTES = {(byte) 0xda, (byte) 0xda};
    private ChannelHandler[] serverHandlers;

    public SeataDetector(ChannelHandler[] serverHandlers) {
        this.serverHandlers = serverHandlers;
    }

    /** 判断可读区magic是否完整匹配，保留原boolean识别含义且不消费输入。 */
    @Override
    public boolean detect(ByteBuf in) {
        if (in.readableBytes() < MAGIC_CODE_BYTES.length) {
            return false;
        }
        for (int i = 0; i < MAGIC_CODE_BYTES.length; i++) {
            if (in.getByte(in.readerIndex() + i) != MAGIC_CODE_BYTES[i]) {
                return false;
            }
        }
        return true;
    }

    /** 版本选择器至少读取magic和版本共3字节；在此之前仅保留匹配magic的前缀。 */
    @Override
    public boolean needsMoreData(ByteBuf in) {
        if (in.readableBytes() >= MAGIC_CODE_BYTES.length + 1) return false;
        for (int i = 0; i < Math.min(in.readableBytes(), MAGIC_CODE_BYTES.length); i++) {
            if (in.getByte(in.readerIndex() + i) != MAGIC_CODE_BYTES[i]) return false;
        }
        return true;
    }

    @Override
    public ChannelHandler[] getHandlers() {
        MultiProtocolDecoder multiProtocolDecoder = new MultiProtocolDecoder(serverHandlers);

        return new ChannelHandler[] {multiProtocolDecoder};
    }
}
