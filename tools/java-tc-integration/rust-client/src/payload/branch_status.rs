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

/// 固定 Java 基线的 BranchStatus 线缆值，未知值拒绝解码。
/// 对应 Java: org.apache.seata.core.model.BranchStatus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum BranchStatus {
    /// 未知分支状态。
    Unknown = 0,
    /// 已向 TC 注册。
    Registered = 1,
    /// 一阶段完成。
    PhaseOneDone = 2,
    /// 一阶段失败。
    PhaseOneFailed = 3,
    /// 一阶段未在超时前报告。
    PhaseOneTimeout = 4,
    /// 二阶段提交成功。
    PhaseTwoCommitted = 5,
    /// 二阶段提交失败且可重试。
    PhaseTwoCommitFailedRetryable = 6,
    /// 二阶段提交失败且不可重试。
    PhaseTwoCommitFailedUnretryable = 7,
    /// 二阶段回滚成功。
    PhaseTwoRollbacked = 8,
    /// 二阶段回滚失败且可重试。
    PhaseTwoRollbackFailedRetryable = 9,
    /// 二阶段回滚失败且不可重试。
    PhaseTwoRollbackFailedUnretryable = 10,
    /// XAER_NOTA 导致提交失败且可重试。
    PhaseTwoCommitFailedXaerNotaRetryable = 11,
    /// XAER_NOTA 导致回滚失败且可重试。
    PhaseTwoRollbackFailedXaerNotaRetryable = 12,
    /// Oracle 预备后仅执行只读查询。
    PhaseOneReadonly = 13,
    /// 用户停止重试。
    StopRetry = 14,
    /// 预备失败，需要通知 TC 全局回滚。
    PhaseOnePrepareFailed = 15,
}

impl BranchStatus {
    /// 输入一个线缆码，返回对应枚举或无效数据错误；不折叠未知值。
    pub(super) fn from_code(code: u8) -> io::Result<Self> {
        match code {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Registered),
            2 => Ok(Self::PhaseOneDone),
            3 => Ok(Self::PhaseOneFailed),
            4 => Ok(Self::PhaseOneTimeout),
            5 => Ok(Self::PhaseTwoCommitted),
            6 => Ok(Self::PhaseTwoCommitFailedRetryable),
            7 => Ok(Self::PhaseTwoCommitFailedUnretryable),
            8 => Ok(Self::PhaseTwoRollbacked),
            9 => Ok(Self::PhaseTwoRollbackFailedRetryable),
            10 => Ok(Self::PhaseTwoRollbackFailedUnretryable),
            11 => Ok(Self::PhaseTwoCommitFailedXaerNotaRetryable),
            12 => Ok(Self::PhaseTwoRollbackFailedXaerNotaRetryable),
            13 => Ok(Self::PhaseOneReadonly),
            14 => Ok(Self::StopRetry),
            15 => Ok(Self::PhaseOnePrepareFailed),
            _ => Err(crate::invalid(format!("unknown BranchStatus: {code}"))),
        }
    }
}
