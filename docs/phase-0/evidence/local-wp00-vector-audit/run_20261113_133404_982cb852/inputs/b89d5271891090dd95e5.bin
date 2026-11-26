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

use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;
use std::io;

/// 独立验证工具的 RegisterTMRequest 字段模型及原生 V1 编解码，不提供生产事务能力。
/// 对应 Java: org.apache.seata.core.protocol.RegisterTMRequest
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegisterTMRequest {
    pub(super) version: String,
    pub(super) application_id: String,
    pub(super) transaction_service_group: String,
    pub(super) extra_data: String,
}

impl RegisterTMRequest {
    /// 输入已定位到消息体的游标，按 Java 字段顺序返回对象或校验错误。
    pub(super) fn read(reader: &mut WireReader<'_>) -> io::Result<Self> {
        Ok(Self {
            version: reader.string16()?,
            application_id: reader.string16()?,
            transaction_service_group: reader.string16()?,
            extra_data: reader.string16()?,
        })
    }

    /// 输入写入器，按 Java V1 宽度编码本对象；返回成功或字段/长度错误。
    pub(super) fn write(&self, writer: &mut WireWriter) -> io::Result<()> {
        writer.string16(&self.version)?;
        writer.string16(&self.application_id)?;
        writer.string16(&self.transaction_service_group)?;
        writer.string16(&self.extra_data)?;
        Ok(())
    }
}
