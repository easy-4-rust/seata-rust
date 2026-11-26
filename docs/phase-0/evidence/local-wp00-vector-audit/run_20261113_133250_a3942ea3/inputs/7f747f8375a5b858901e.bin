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

use super::branch_type::BranchType;
use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;
use std::io;

/// 独立验证工具的 BranchRollbackRequest 字段模型及原生 V1 编解码，不提供生产事务能力。
/// 对应 Java: org.apache.seata.core.protocol.transaction.BranchRollbackRequest
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BranchRollbackRequest {
    pub(super) xid: String,
    pub(super) branch_id: i64,
    pub(super) branch_type: BranchType,
    pub(super) resource_id: String,
    pub(super) application_data: Option<String>,
}

impl BranchRollbackRequest {
    /// 输入已定位到消息体的游标，按 Java 字段顺序返回对象或校验错误。
    pub(super) fn read(reader: &mut WireReader<'_>) -> io::Result<Self> {
        Ok(Self {
            xid: reader.required16()?,
            branch_id: reader.i64()?,
            branch_type: BranchType::from_code(reader.u8()?)?,
            resource_id: reader.required16()?,
            application_data: reader.optional32()?,
        })
    }

    /// 输入写入器，按 Java V1 宽度编码本对象；返回成功或字段/长度错误。
    pub(super) fn write(&self, writer: &mut WireWriter) -> io::Result<()> {
        writer.required16(&self.xid)?;
        writer.i64(self.branch_id)?;
        writer.u8(self.branch_type as u8)?;
        writer.required16(&self.resource_id)?;
        writer.optional32(self.application_data.as_deref())?;
        Ok(())
    }
}
