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

import org.apache.seata.common.loader.EnhancedServiceLoader;
import org.apache.seata.core.protocol.RegisterRMRequest;
import org.apache.seata.core.protocol.RegisterTMRequest;
import org.apache.seata.core.rpc.RegisterCheckAuthHandler;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 认证扩展规则与默认允许策略的差异；真实开关和通道绑定另由现场验证。 */
class Phase0CredentialCheckAuthHandlerTest {
    private final Phase0CredentialCheckAuthHandler handler = new Phase0CredentialCheckAuthHandler();

    @Test
    void actualServiceLoaderSelectsTheTestPolicyOverThePermissiveDefault(@TempDir Path directory) throws Exception {
        Path service = directory.resolve("META-INF/services/" + RegisterCheckAuthHandler.class.getName());
        Files.createDirectories(service.getParent());
        Files.writeString(
                service,
                Phase0CredentialCheckAuthHandler.class.getName() + "\n" + DefaultCheckAuthHandler.class.getName()
                        + "\n");
        EnhancedServiceLoader.unload(RegisterCheckAuthHandler.class);
        try (URLClassLoader loader = new URLClassLoader(
                new URL[] {directory.toUri().toURL()}, getClass().getClassLoader())) {
            RegisterCheckAuthHandler selected = EnhancedServiceLoader.load(RegisterCheckAuthHandler.class, loader);
            assertTrue(selected instanceof Phase0CredentialCheckAuthHandler);
        } finally {
            EnhancedServiceLoader.unload(RegisterCheckAuthHandler.class);
        }
    }

    @Test
    void acceptsTheExactTestIdentityCredentialAndCachedResources() {
        assertTrue(handler.doRegTransactionManagerCheck(tm("phase0-local-only")));
        assertTrue(handler.doRegResourceManagerCheck(rm("phase0-local-only")));
    }

    @Test
    void missingAndWrongCredentialsAreDeniedForBothRoles() {
        for (String credential : new String[] {null, "", "wrong", "phase0-local-only-extra"}) {
            assertFalse(handler.doRegTransactionManagerCheck(tm(credential)));
            assertFalse(handler.doRegResourceManagerCheck(rm(credential)));
        }
    }

    @Test
    void validCredentialDoesNotAuthorizeAnotherIdentityOrGroup() {
        RegisterTMRequest tm = tm("phase0-local-only");
        tm.setApplicationId("another-app");
        assertFalse(handler.doRegTransactionManagerCheck(tm));
        tm = tm("phase0-local-only");
        tm.setTransactionServiceGroup("another-group");
        assertFalse(handler.doRegTransactionManagerCheck(tm));
        RegisterRMRequest rm = rm("phase0-local-only");
        rm.setApplicationId("another-app");
        assertFalse(handler.doRegResourceManagerCheck(rm));
        rm = rm("phase0-local-only");
        rm.setTransactionServiceGroup("another-group");
        assertFalse(handler.doRegResourceManagerCheck(rm));
    }

    @Test
    void everyResourceMustBeOnTheExactAllowlist() {
        for (String resources :
                new String[] {null, "", "mysql://other", "mysql://auth-a,mysql://other", "mysql://auth-a,"}) {
            RegisterRMRequest request = rm("phase0-local-only");
            request.setResourceIds(resources);
            assertFalse(handler.doRegResourceManagerCheck(request));
        }
    }

    @Test
    void injectedProviderFailureRemainsAnExceptionForTheRealProcessorToMap() {
        assertThrows(IllegalStateException.class, () -> handler.doRegTransactionManagerCheck(tm("phase0-throw")));
        assertThrows(IllegalStateException.class, () -> handler.doRegResourceManagerCheck(rm("phase0-throw")));
    }

    @Test
    void originalDefaultProviderIsPermissiveEvenWithoutCredentials() {
        DefaultCheckAuthHandler original = new DefaultCheckAuthHandler();
        assertTrue(original.doRegTransactionManagerCheck(tm(null)));
        assertTrue(original.doRegResourceManagerCheck(rm(null)));
    }

    private RegisterTMRequest tm(String credential) {
        RegisterTMRequest request = new RegisterTMRequest();
        request.setApplicationId("phase0-auth");
        request.setTransactionServiceGroup("default_tx_group");
        request.setExtraData(credential);
        return request;
    }

    private RegisterRMRequest rm(String credential) {
        RegisterRMRequest request = new RegisterRMRequest();
        request.setApplicationId("phase0-auth");
        request.setTransactionServiceGroup("default_tx_group");
        request.setExtraData(credential);
        request.setResourceIds("mysql://auth-a,mysql://auth-b");
        return request;
    }
}
