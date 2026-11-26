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

/// 固定 Java 基线的 ResultCode 线缆值，未知值拒绝解码。
/// 对应 Java: org.apache.seata.core.protocol.ResultCode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum ResultCode {
    /// 明确失败，仍需保留响应内的状态。
    Failed = 0,
    /// 明确成功。
    Success = 1,
}

impl ResultCode {
    /// 输入一个线缆码，返回对应枚举或无效数据错误；不折叠未知值。
    pub(super) fn from_code(code: u8) -> io::Result<Self> {
        match code {
            0 => Ok(Self::Failed),
            1 => Ok(Self::Success),
            _ => Err(crate::invalid(format!("unknown ResultCode: {code}"))),
        }
    }
}
