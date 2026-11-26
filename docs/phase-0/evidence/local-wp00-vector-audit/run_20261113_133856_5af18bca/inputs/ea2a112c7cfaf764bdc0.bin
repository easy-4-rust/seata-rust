// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements. See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License. You may obtain a copy of the License at
// http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io;

/// 固定 Java 基线的 GlobalStatus 线缆值，未知值拒绝解码。
/// 对应 Java: org.apache.seata.core.model.GlobalStatus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum GlobalStatus {
    /// 事务开始前的模糊状态。
    Unknown = 0,
    /// 全局事务开始，可注册分支。
    Begin = 1,
    /// 二阶段提交中。
    Committing = 2,
    /// 提交可恢复失败后重试。
    CommitRetrying = 3,
    /// 二阶段回滚中。
    Rollbacking = 4,
    /// 回滚可恢复失败后重试。
    RollbackRetrying = 5,
    /// 超时触发回滚。
    TimeoutRollbacking = 6,
    /// 超时回滚失败后重试。
    TimeoutRollbackRetrying = 7,
    /// 全部分支可异步提交，尚未实际完成，客户端可视为提交。
    AsyncCommitting = 8,
    /// 全局事务提交成功，终态。
    Committed = 9,
    /// 二阶段提交最终失败。
    CommitFailed = 10,
    /// 全局事务回滚成功，终态。
    Rollbacked = 11,
    /// 全局事务回滚最终失败。
    RollbackFailed = 12,
    /// 超时回滚成功，终态。
    TimeoutRollbacked = 13,
    /// 超时回滚最终失败。
    TimeoutRollbackFailed = 14,
    /// 会话已不在管理集合；无事务和 Saga 报告都可使用，不能据此证明提交。
    Finished = 15,
    /// 提交重试达到时限。
    CommitRetryTimeout = 16,
    /// 回滚重试达到时限。
    RollbackRetryTimeout = 17,
    /// 正在删除事务。
    Deleting = 18,
    /// 停止提交或提交重试。
    StopCommitOrCommitRetry = 19,
    /// 停止回滚或回滚重试。
    StopRollbackOrRollbackRetry = 20,
}

impl GlobalStatus {
    /// 输入一个线缆码，返回对应枚举或无效数据错误；不折叠未知值。
    pub(super) fn from_code(code: u8) -> io::Result<Self> {
        match code {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Begin),
            2 => Ok(Self::Committing),
            3 => Ok(Self::CommitRetrying),
            4 => Ok(Self::Rollbacking),
            5 => Ok(Self::RollbackRetrying),
            6 => Ok(Self::TimeoutRollbacking),
            7 => Ok(Self::TimeoutRollbackRetrying),
            8 => Ok(Self::AsyncCommitting),
            9 => Ok(Self::Committed),
            10 => Ok(Self::CommitFailed),
            11 => Ok(Self::Rollbacked),
            12 => Ok(Self::RollbackFailed),
            13 => Ok(Self::TimeoutRollbacked),
            14 => Ok(Self::TimeoutRollbackFailed),
            15 => Ok(Self::Finished),
            16 => Ok(Self::CommitRetryTimeout),
            17 => Ok(Self::RollbackRetryTimeout),
            18 => Ok(Self::Deleting),
            19 => Ok(Self::StopCommitOrCommitRetry),
            20 => Ok(Self::StopRollbackOrRollbackRetry),
            _ => Err(crate::invalid(format!("unknown GlobalStatus: {code}"))),
        }
    }
}
