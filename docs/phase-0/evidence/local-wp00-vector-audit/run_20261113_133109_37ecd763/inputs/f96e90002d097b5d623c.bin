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

/// 有界、严格的 V1 字段游标；先核对长度，再取得字节并分配字符串。
/// 对应 Java: 各 Seata V1 Codec 的 ByteBuffer 读取职责（严格错误处理为探针策略）。
pub(super) struct WireReader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> WireReader<'a> {
    /// 输入完整载荷，返回游标；超过探针上限时拒绝。
    pub(super) fn new(input: &'a [u8]) -> io::Result<Self> {
        if input.len() > crate::MAX_FRAME_LEN {
            return Err(crate::invalid("payload exceeds probe limit"));
        }
        Ok(Self { input, offset: 0 })
    }

    /// 输入字节数，返回有界切片并推进游标；不足时拒绝。
    pub(super) fn take(&mut self, len: usize) -> io::Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .filter(|end| *end <= self.input.len())
            .ok_or_else(|| crate::invalid("truncated payload field"))?;
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    /// 无参数，返回尚未消费的字节数。
    pub(super) fn remaining(&self) -> usize {
        self.input.len() - self.offset
    }

    /// 无参数，观察下一类型前缀但不推进游标，截断时拒绝。
    pub(super) fn peek_u16(&self) -> io::Result<u16> {
        if self.remaining() < 2 {
            return Err(crate::invalid("missing nested type"));
        }
        Ok(u16::from_be_bytes([
            self.input[self.offset],
            self.input[self.offset + 1],
        ]))
    }

    /// 无参数，读取一个字节；不足时返回错误。
    pub(super) fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    /// 无参数，读取大端无符号短整数；不足时返回错误。
    pub(super) fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| crate::invalid("short width"))?,
        ))
    }
    /// 无参数，读取 Java short；保留符号，不进行业务校验。
    pub(super) fn i16(&mut self) -> io::Result<i16> {
        Ok(i16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| crate::invalid("short width"))?,
        ))
    }
    /// 无参数，读取 Java int；不足时返回错误。
    pub(super) fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| crate::invalid("int width"))?,
        ))
    }
    /// 无参数，读取 Java long；不足时返回错误。
    pub(super) fn i64(&mut self) -> io::Result<i64> {
        Ok(i64::from_be_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| crate::invalid("long width"))?,
        ))
    }

    fn text(&mut self, len: usize) -> io::Result<String> {
        std::str::from_utf8(self.take(len)?)
            .map(str::to_owned)
            .map_err(|_| crate::invalid("invalid UTF-8"))
    }
    /// 无参数，读取 Java 有符号 short 长度字符串；零长度返回 None。
    pub(super) fn optional16(&mut self) -> io::Result<Option<String>> {
        let len =
            usize::try_from(self.i16()?).map_err(|_| crate::invalid("negative string16 length"))?;
        if len == 0 {
            Ok(None)
        } else {
            self.text(len).map(Some)
        }
    }
    /// 无参数，读取零长度仍调用 setter 的 Java 字段；返回空字符串或 UTF-8 内容。
    pub(super) fn string16(&mut self) -> io::Result<String> {
        Ok(self.optional16()?.unwrap_or_default())
    }
    /// 无参数，读取 Java 提前返回分支中必需的非空字段；不接受部分对象。
    pub(super) fn required16(&mut self) -> io::Result<String> {
        self.optional16()?
            .ok_or_else(|| crate::invalid("required nonempty string16 missing"))
    }
    /// 无参数，读取 Java int 长度字符串；负数/截断拒绝，零长度返回 None。
    pub(super) fn optional32(&mut self) -> io::Result<Option<String>> {
        let len =
            usize::try_from(self.i32()?).map_err(|_| crate::invalid("negative string32 length"))?;
        if len == 0 {
            Ok(None)
        } else {
            self.text(len).map(Some)
        }
    }
    /// 无参数，读取严格 0/1 单字节布尔值；其他值拒绝。
    pub(super) fn bool8(&mut self) -> io::Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(crate::invalid("invalid bool8")),
        }
    }
    /// 无参数，读取锁查询的严格 0/1 双字节布尔值；其他值拒绝。
    pub(super) fn bool16(&mut self) -> io::Result<bool> {
        match self.u16()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(crate::invalid("invalid bool16")),
        }
    }
    /// 无参数，要求整个输入已消费；多余数据返回错误。
    pub(super) fn finish(self) -> io::Result<()> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(crate::invalid("trailing payload bytes"))
        }
    }
}
