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

/**
 * 可选的分片探测能力，保留原ProtocolDetector二进制/源码接口。
 * 仅当当前可读区仍是合法前缀但不足安全交给协议选择器时返回true；不得保留或释放输入视图。
 */
public interface IncrementalProtocolDetector extends ProtocolDetector {
    /** 判断是否应保留当前前缀等待后续数据；输入为独立索引只读视图。 */
    boolean needsMoreData(ByteBuf in);
}
