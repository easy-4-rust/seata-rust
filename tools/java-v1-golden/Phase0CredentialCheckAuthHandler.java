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
package org.apache.seata.server.auth;

import org.apache.seata.common.loader.LoadLevel;
import org.apache.seata.core.protocol.AbstractIdentifyRequest;
import org.apache.seata.core.protocol.RegisterRMRequest;
import org.apache.seata.core.protocol.RegisterTMRequest;
import org.springframework.util.StringUtils;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Objects;

/** 本地联调专用认证 SPI；固定测试凭据，不是生产安全方案，不修改原 TC。 */
@LoadLevel(name = "phase0CredentialCheckAuthHandler", order = 200)
public final class Phase0CredentialCheckAuthHandler extends AbstractCheckAuthHandler {
    /** 校验 TM 注册请求中的测试身份及凭据，返回是否允许。 */
    @Override
    public boolean doRegTransactionManagerCheck(RegisterTMRequest request) {
        return identity(request);
    }

    /** 校验 RM 注册请求及资源白名单，返回是否允许。 */
    @Override
    public boolean doRegResourceManagerCheck(RegisterRMRequest request) {
        if (!identity(request) || !StringUtils.hasText(request.getResourceIds())) {
            return false;
        }
        for (String resource : StringUtils.delimitedListToStringArray(request.getResourceIds(), ",")) {
            if (!"mysql://auth-a".equals(resource) && !"mysql://auth-b".equals(resource)) {
                return false;
            }
        }
        return true;
    }

    private boolean identity(AbstractIdentifyRequest request) {
        if (Objects.isNull(request)
                || !"phase0-auth".equals(request.getApplicationId())
                || !"default_tx_group".equals(request.getTransactionServiceGroup())) {
            return false;
        }
        String credential = request.getExtraData();
        if ("phase0-throw".equals(credential)) {
            throw new IllegalStateException("phase0 injected auth failure");
        }
        return StringUtils.hasText(credential)
                && MessageDigest.isEqual(
                        "phase0-local-only".getBytes(StandardCharsets.UTF_8),
                        credential.getBytes(StandardCharsets.UTF_8));
    }
}
