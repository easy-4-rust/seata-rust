package org.apache.seata.server.auth;

import org.apache.seata.common.loader.LoadLevel;
import org.apache.seata.core.protocol.RegisterRMRequest;
import org.apache.seata.core.protocol.RegisterTMRequest;

import java.util.Objects;

/** 仅用于自有原 TC 实例，允许本轮两个精确的 JDBC 资源 ID。 */
@LoadLevel(name = "resourceAliasAuthHandler", order = 201)
public final class ResourceAliasAuthHandler extends AbstractCheckAuthHandler {
    private final Phase0CredentialCheckAuthHandler identityChecker = new Phase0CredentialCheckAuthHandler();

    /** 校验本地 TM 测试身份。 */
    @Override
    public boolean doRegTransactionManagerCheck(RegisterTMRequest request) {
        return identityChecker.doRegTransactionManagerCheck(request);
    }

    /** 对本地 RM 校验身份和精确资源 ID。 */
    @Override
    public boolean doRegResourceManagerCheck(RegisterRMRequest request) {
        if (Objects.isNull(request) || Objects.isNull(request.getResourceIds())) {
            return false;
        }
        String resource = request.getResourceIds();
        if (!resource.equals(System.getProperty("seata.callback.fixture.resource"))
                && !resource.equals(System.getProperty("seata.callback.fixture.alias"))) {
            return false;
        }
        RegisterTMRequest identity = new RegisterTMRequest();
        identity.setApplicationId(request.getApplicationId());
        identity.setTransactionServiceGroup(request.getTransactionServiceGroup());
        identity.setExtraData(request.getExtraData());
        return doRegTransactionManagerCheck(identity);
    }
}
