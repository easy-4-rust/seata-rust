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
package org.apache.seata.core.rpc.processor.server;

import io.netty.channel.Channel;
import io.netty.channel.ChannelHandlerContext;
import org.apache.seata.common.loader.EnhancedServiceLoader;
import org.apache.seata.core.protocol.AbstractIdentifyRequest;
import org.apache.seata.core.protocol.AbstractIdentifyResponse;
import org.apache.seata.core.protocol.RegisterRMRequest;
import org.apache.seata.core.protocol.RegisterTMRequest;
import org.apache.seata.core.protocol.ResultCode;
import org.apache.seata.core.protocol.RpcMessage;
import org.apache.seata.core.protocol.Version;
import org.apache.seata.core.rpc.RegisterCheckAuthHandler;
import org.apache.seata.core.rpc.RemotingServer;
import org.apache.seata.core.rpc.netty.ChannelManager;
import org.apache.seata.core.rpc.processor.RemotingProcessor;
import org.apache.seata.serializer.seata.HandlerWireRecord;
import org.apache.seata.serializer.seata.SeataSerializer;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;
import org.mockito.ArgumentCaptor;
import org.mockito.MockedStatic;

import java.net.InetSocketAddress;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.mockito.ArgumentMatchers.same;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.mockStatic;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.verifyNoMoreInteractions;
import static org.mockito.Mockito.when;

/** 运行真实注册处理器，注入 auth hook 结果并核对通道注册调用及 V1 实际应答。 */
class RegistrationProcessorAuthTest {
    private enum Auth { ALLOW, DENY, THROW }

    @TestFactory
    List<DynamicTest> tmAndRmAuthAllowDenyAndException() {
        List<DynamicTest> tests = new ArrayList<>();
        for (boolean tm : new boolean[] {true, false}) {
            for (Auth auth : Auth.values()) {
                tests.add(DynamicTest.dynamicTest((tm ? "TM/" : "RM/") + auth, () -> verifyCase(tm, auth, false)));
            }
        }
        return tests;
    }

    @Test
    void deniedClosedRmChannelIsCleanedWithoutResourceRegistration() throws Exception {
        verifyCase(false, Auth.DENY, true);
    }

    private static void verifyCase(boolean tm, Auth behavior, boolean closed) throws Exception {
        String current = Version.getCurrent();
        AbstractIdentifyRequest request = tm ? new RegisterTMRequest() : new RegisterRMRequest();
        request.setApplicationId("orders-auth-test");
        request.setTransactionServiceGroup("default_tx_group");
        request.setVersion("1.1.0");
        if (!tm) { ((RegisterRMRequest) request).setResourceIds("mysql://orders"); }
        RegisterCheckAuthHandler auth = mock(RegisterCheckAuthHandler.class);
        if (tm) {
            if (behavior == Auth.THROW) { when(auth.regTransactionManagerCheckAuth((RegisterTMRequest) request)).thenThrow(new IllegalStateException("auth crashed")); }
            else { when(auth.regTransactionManagerCheckAuth((RegisterTMRequest) request)).thenReturn(behavior == Auth.ALLOW); }
        } else {
            if (behavior == Auth.THROW) { when(auth.regResourceManagerCheckAuth((RegisterRMRequest) request)).thenThrow(new IllegalStateException("auth crashed")); }
            else { when(auth.regResourceManagerCheckAuth((RegisterRMRequest) request)).thenReturn(behavior == Auth.ALLOW); }
        }
        RemotingServer server = mock(RemotingServer.class);
        RemotingProcessor processor;
        // 只在构造器内替换 SPI 加载，运行 process 时恢复其余真实服务加载逻辑。
        try (MockedStatic<EnhancedServiceLoader> loader = mockStatic(EnhancedServiceLoader.class)) {
            loader.when(() -> EnhancedServiceLoader.load(RegisterCheckAuthHandler.class)).thenReturn(auth);
            processor = tm ? new RegTmProcessor(server) : new RegRmProcessor(server);
        }
        Channel channel = mock(Channel.class);
        when(channel.remoteAddress()).thenReturn(new InetSocketAddress("127.0.0.1", tm ? 29101 : 29103));
        when(channel.isActive()).thenReturn(!closed);
        ChannelHandlerContext ctx = mock(ChannelHandlerContext.class);
        when(ctx.channel()).thenReturn(channel);
        RpcMessage rpc = new RpcMessage();
        rpc.setBody(request);
        try (MockedStatic<ChannelManager> channels = mockStatic(ChannelManager.class);
                MockedStatic<Version> versions = mockStatic(Version.class)) {
            versions.when(Version::getCurrent).thenReturn(current);
            processor.process(ctx, rpc);
            boolean allowed = behavior == Auth.ALLOW;
            ArgumentCaptor<AbstractIdentifyResponse> captured = ArgumentCaptor.forClass(AbstractIdentifyResponse.class);
            verify(server).sendAsyncResponse(same(rpc), same(channel), captured.capture());
            verifyNoMoreInteractions(server);
            AbstractIdentifyResponse response = captured.getValue();
            assertEquals(allowed, response.isIdentified());
            assertEquals(allowed ? ResultCode.Success : ResultCode.Failed, response.getResultCode());
            assertEquals(behavior == Auth.ALLOW ? null : behavior == Auth.THROW ? "auth crashed" : tm ? "TM checkAuth fail" : "RM checkAuth fail", response.getMsg());
            assertEquals(current, response.getVersion());
            if (allowed && tm) { channels.verify(() -> ChannelManager.registerTMChannel((RegisterTMRequest) request, channel)); }
            if (allowed && !tm) { channels.verify(() -> ChannelManager.registerRMChannel((RegisterRMRequest) request, channel)); }
            if (closed && !tm) { channels.verify(() -> ChannelManager.removeClosedChannel(channel)); }
            channels.verifyNoMoreInteractions();
            if (tm) { verify(auth).regTransactionManagerCheckAuth((RegisterTMRequest) request); }
            else { verify(auth).regResourceManagerCheckAuth((RegisterRMRequest) request); }
            verifyNoMoreInteractions(auth);
            versions.verify(() -> Version.putChannelVersion(channel, "1.1.0"), times(tm ? allowed ? 2 : 1 : allowed ? 1 : 0));

            SeataSerializer serializer = new SeataSerializer((byte) 1);
            byte[] bytes = serializer.serialize(response);
            assertEquals(0, bytes[0]);
            assertEquals(tm ? 102 : 104, bytes[1] & 255);
            assertEquals(allowed ? 1 : 0, bytes[2]);
            AbstractIdentifyResponse decoded = serializer.deserialize(bytes);
            assertEquals(allowed, decoded.isIdentified());
            assertEquals(current, decoded.getVersion());
            assertNull(decoded.getMsg(), "V1 registration body does not transmit the Java error text");
            // 新建响应默认 Success，IdentifyResponseCodec 不读写 ResultCode，拒绝必须依据 identified。
            assertEquals(ResultCode.Success, decoded.getResultCode());
            if (!allowed) { assertFalse(decoded.isIdentified()); }
            HandlerWireRecord.emit("auth/" + (tm ? "TM/" : "RM/") + (closed ? "DENY_CLOSED" : behavior.name()),
                    tm ? "REG_TM" : "REG_RM", bytes, allowed ? 1 : 0, 0, "Unknown", null, -1, null, 0L, false, current);
        }
    }
}
