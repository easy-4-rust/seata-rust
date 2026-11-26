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

/// 独立验证工具的 RegisterRMResponse 字段模型及原生 V1 编解码，不提供生产事务能力。
/// 对应 Java: org.apache.seata.core.protocol.RegisterRMResponse
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegisterRMResponse {
    pub(super) identified: bool,
    /// 线缆版本字段是否存在；None 不等于 Java 对象的 version=null。
    pub(super) wire_version: Option<String>,
}

impl RegisterRMResponse {
    /// 输入已定位到消息体的游标，按 Java 字段顺序返回对象或校验错误。
    pub(super) fn read(reader: &mut WireReader<'_>) -> io::Result<Self> {
        Ok(Self {
            identified: reader.bool8()?,
            wire_version: reader.optional16()?,
        })
    }

    /// 输入写入器，按 Java V1 宽度编码本对象；返回成功或字段/长度错误。
    pub(super) fn write(&self, writer: &mut WireWriter) -> io::Result<()> {
        writer.u8(u8::from(self.identified))?;
        writer.optional16(self.wire_version.as_deref())?;
        Ok(())
    }

    /// 输入 Java 构建的当前版本，返回 Java getVersion 语义；缺省字段保留构造默认值。
    /// 对应 Java: AbstractIdentifyResponse#getVersion、AbstractIdentifyResponseCodec#decode。
    pub(super) fn java_version<'a>(&'a self, java_current: &'a str) -> &'a str {
        self.wire_version.as_deref().unwrap_or(java_current)
    }
}
