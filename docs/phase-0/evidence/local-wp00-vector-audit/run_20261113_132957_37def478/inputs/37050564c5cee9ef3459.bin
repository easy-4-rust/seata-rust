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

/// 固定 Java 基线的 TransactionExceptionCode 线缆值，未知值拒绝解码。
/// 对应 Java: org.apache.seata.core.exception.TransactionExceptionCode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum TransactionExceptionCode {
    /// 未知事务异常。
    Unknown = 0,
    /// 开启失败。
    BeginFailed = 1,
    /// 锁键冲突。
    LockKeyConflict = 2,
    /// 输入输出异常。
    Io = 3,
    /// 分支回滚失败可重试。
    BranchRollbackFailedRetriable = 4,
    /// 分支回滚失败不可重试。
    BranchRollbackFailedUnretriable = 5,
    /// 分支注册失败。
    BranchRegisterFailed = 6,
    /// 分支报告失败。
    BranchReportFailed = 7,
    /// 锁检查失败。
    LockableCheckFailed = 8,
    /// 分支事务不存在。
    BranchTransactionNotExist = 9,
    /// 全局事务不存在。
    GlobalTransactionNotExist = 10,
    /// 全局事务非活动。
    GlobalTransactionNotActive = 11,
    /// 全局事务状态无效。
    GlobalTransactionStatusInvalid = 12,
    /// 发送分支提交请求失败。
    FailedToSendBranchCommitRequest = 13,
    /// 发送分支回滚请求失败。
    FailedToSendBranchRollbackRequest = 14,
    /// 添加分支失败。
    FailedToAddBranch = 15,
    /// 锁定全局事务失败。
    FailedLockGlobalTransaction = 16,
    /// 写会话失败。
    FailedWriteSession = 17,
    /// 存储失败。
    FailedStore = 18,
    /// 并非 Raft 领导者。
    NotRaftLeader = 19,
    /// 锁冲突快速失败。
    LockKeyConflictFailFast = 20,
    /// 事务超时。
    TransactionTimeout = 21,
    /// 启发式提交结果。
    CommitHeuristic = 22,
    /// 事务已损坏。
    Broken = 23,
    /// 分支预备失败。
    BranchPrepareFailed = 24,
}

impl TransactionExceptionCode {
    /// 输入一个线缆码，返回对应枚举或无效数据错误；不折叠未知值。
    pub(super) fn from_code(code: u8) -> io::Result<Self> {
        match code {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::BeginFailed),
            2 => Ok(Self::LockKeyConflict),
            3 => Ok(Self::Io),
            4 => Ok(Self::BranchRollbackFailedRetriable),
            5 => Ok(Self::BranchRollbackFailedUnretriable),
            6 => Ok(Self::BranchRegisterFailed),
            7 => Ok(Self::BranchReportFailed),
            8 => Ok(Self::LockableCheckFailed),
            9 => Ok(Self::BranchTransactionNotExist),
            10 => Ok(Self::GlobalTransactionNotExist),
            11 => Ok(Self::GlobalTransactionNotActive),
            12 => Ok(Self::GlobalTransactionStatusInvalid),
            13 => Ok(Self::FailedToSendBranchCommitRequest),
            14 => Ok(Self::FailedToSendBranchRollbackRequest),
            15 => Ok(Self::FailedToAddBranch),
            16 => Ok(Self::FailedLockGlobalTransaction),
            17 => Ok(Self::FailedWriteSession),
            18 => Ok(Self::FailedStore),
            19 => Ok(Self::NotRaftLeader),
            20 => Ok(Self::LockKeyConflictFailFast),
            21 => Ok(Self::TransactionTimeout),
            22 => Ok(Self::CommitHeuristic),
            23 => Ok(Self::Broken),
            24 => Ok(Self::BranchPrepareFailed),
            _ => Err(crate::invalid(format!(
                "unknown TransactionExceptionCode: {code}"
            ))),
        }
    }
}
