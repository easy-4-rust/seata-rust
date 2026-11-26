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
package org.apache.seata.server.cluster.raft.sync.msg.dto;

import org.apache.seata.core.store.BranchTransactionDO;
import com.fasterxml.jackson.annotation.JsonInclude;

/**
 */
public class BranchTransactionDTO extends BranchTransactionDO {

    private static final long serialVersionUID = 4550610938263777969L;
    private String lockKey;

    /** true表示本次状态消息显式更新上下文；缺失/null/false保留历史status-only语义。 */
    private Boolean applicationDataUpdated;

    /** 返回本次状态消息是否显式更新上下文；null表示历史消息未指定。 */
    @JsonInclude(JsonInclude.Include.NON_NULL)
    public Boolean getApplicationDataUpdated() {
        return applicationDataUpdated;
    }

    /** 设置可空更新标志；参数true同时允许applicationData为null以明确清除。 */
    public void setApplicationDataUpdated(Boolean applicationDataUpdated) {
        this.applicationDataUpdated = applicationDataUpdated;
    }

    public BranchTransactionDTO(String xid, long branchId) {
        super(xid, branchId);
    }

    public BranchTransactionDTO() {
        super();
    }

    public String getLockKey() {
        return lockKey;
    }

    public void setLockKey(String lockKey) {
        this.lockKey = lockKey;
    }
}
