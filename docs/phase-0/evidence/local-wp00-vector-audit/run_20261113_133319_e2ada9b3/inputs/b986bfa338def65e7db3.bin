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

/// V1 字段写入器，验证长度后写入，避免窄化转换和超上限载荷。
/// 对应 Java: 各 Seata V1 Codec 的 ByteBuf 写入职责。
#[derive(Default)]
pub(super) struct WireWriter {
    bytes: Vec<u8>,
}

impl WireWriter {
    /// 输入原始嵌套消息字节，验证总上限后写入。
    pub(super) fn raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > crate::MAX_FRAME_LEN.saturating_sub(self.bytes.len()) {
            return Err(crate::invalid("payload exceeds probe limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    /// 输入单字节字段并写入，返回成功或载荷上限错误。
    pub(super) fn u8(&mut self, value: u8) -> io::Result<()> {
        self.raw(&[value])
    }
    /// 输入大端双字节字段并写入，返回成功或上限错误。
    pub(super) fn u16(&mut self, value: u16) -> io::Result<()> {
        self.raw(&value.to_be_bytes())
    }
    /// 输入 Java short 并保留符号写入，返回成功或上限错误。
    pub(super) fn i16(&mut self, value: i16) -> io::Result<()> {
        self.raw(&value.to_be_bytes())
    }
    /// 输入 Java int 并写入，返回成功或上限错误。
    pub(super) fn i32(&mut self, value: i32) -> io::Result<()> {
        self.raw(&value.to_be_bytes())
    }
    /// 输入 Java long 并写入，返回成功或上限错误。
    pub(super) fn i64(&mut self, value: i64) -> io::Result<()> {
        self.raw(&value.to_be_bytes())
    }
    /// 输入可选文本，零长度编码为 short 0；返回成功或 Java short 字节长度错误。
    pub(super) fn optional16(&mut self, value: Option<&str>) -> io::Result<()> {
        let bytes = value.unwrap_or_default().as_bytes();
        let len = i16::try_from(bytes.len())
            .map_err(|_| crate::invalid("string16 exceeds Java signed short"))?;
        self.i16(len)?;
        self.raw(bytes)
    }
    /// 输入文本并写入 short 长度字段；返回成功或长度错误。
    pub(super) fn string16(&mut self, value: &str) -> io::Result<()> {
        self.optional16(Some(value))
    }
    /// 输入必需的非空文本，空值拒绝；返回成功或长度错误。
    pub(super) fn required16(&mut self, value: &str) -> io::Result<()> {
        if value.is_empty() {
            return Err(crate::invalid("required nonempty string16 missing"));
        }
        self.string16(value)
    }
    /// 输入可选文本并写入 int 字节长度；返回成功或长度/载荷上限错误。
    pub(super) fn optional32(&mut self, value: Option<&str>) -> io::Result<()> {
        let bytes = value.unwrap_or_default().as_bytes();
        let len =
            i32::try_from(bytes.len()).map_err(|_| crate::invalid("string32 exceeds Java int"))?;
        self.i32(len)?;
        self.raw(bytes)
    }
    /// 无参数，取出已写入的完整字节；调用者处理后续传输。
    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
