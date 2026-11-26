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

use super::abstract_transaction_response::AbstractTransactionResponse;
use super::branch_status::BranchStatus;
use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;
use std::io;

/// 独立验证工具的 BranchRollbackResponse 字段模型及原生 V1 编解码，不提供生产事务能力。
/// 对应 Java: org.apache.seata.core.protocol.transaction.BranchRollbackResponse
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BranchRollbackResponse {
    pub(super) response: AbstractTransactionResponse,
    pub(super) xid: Option<String>,
    pub(super) branch_id: i64,
    pub(super) branch_status: BranchStatus,
}

impl BranchRollbackResponse {
    /// 输入已定位到消息体的游标，按 Java 字段顺序返回对象或校验错误。
    pub(super) fn read(reader: &mut WireReader<'_>) -> io::Result<Self> {
        Ok(Self {
            response: AbstractTransactionResponse::read(reader)?,
            xid: reader.optional16()?,
            branch_id: reader.i64()?,
            branch_status: BranchStatus::from_code(reader.u8()?)?,
        })
    }

    /// 输入写入器，按 Java V1 宽度编码本对象；返回成功或字段/长度错误。
    pub(super) fn write(&self, writer: &mut WireWriter) -> io::Result<()> {
        self.response.write(writer)?;
        writer.optional16(self.xid.as_deref())?;
        writer.i64(self.branch_id)?;
        writer.u8(self.branch_status as u8)?;
        Ok(())
    }
}
