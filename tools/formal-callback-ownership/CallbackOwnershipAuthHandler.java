package org.apache.seata.server.auth;

import org.apache.seata.common.loader.LoadLevel;
import org.apache.seata.core.protocol.RegisterRMRequest;
import org.apache.seata.core.protocol.RegisterTMRequest;
import org.springframework.util.StringUtils;

import java.util.Objects;

/** 自有回调测试实例的精确资源认证；保留已认证夹具的身份和凭据规则，不是生产实现。 */
@LoadLevel(name = "callbackOwnershipAuthHandler", order = 201)
public final class CallbackOwnershipAuthHandler extends AbstractCheckAuthHandler {
    private final Phase0CredentialCheckAuthHandler identityChecker = new Phase0CredentialCheckAuthHandler();

    /** 参数为原 TM 请求；返回既有固定测试身份规则的结果。 */
    @Override
    public boolean doRegTransactionManagerCheck(RegisterTMRequest request) {
        return Objects.nonNull(request) && identityChecker.doRegTransactionManagerCheck(request);
    }

    /** 参数为原 RM 请求；仅接受当前实例指定的一个完整资源名及原身份。 */
    @Override
    public boolean doRegResourceManagerCheck(RegisterRMRequest request) {
        String expectedResource = System.getProperty("seata.callback.fixture.resource");
        if (Objects.isNull(request) || !StringUtils.hasText(expectedResource)
                || !expectedResource.equals(request.getResourceIds())) {
            return false;
        }
        RegisterTMRequest identity = new RegisterTMRequest();
        identity.setApplicationId(request.getApplicationId());
        identity.setTransactionServiceGroup(request.getTransactionServiceGroup());
        identity.setExtraData(request.getExtraData());
        return identityChecker.doRegTransactionManagerCheck(identity);
    }
}
