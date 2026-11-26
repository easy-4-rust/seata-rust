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

use super::result_code::ResultCode;
use super::transaction_exception_code::TransactionExceptionCode;
use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;
use std::io;

/// 事务响应共有的结果、失败消息和异常码；失败不丢弃后续状态。
/// 对应 Java: org.apache.seata.core.protocol.transaction.AbstractTransactionResponse
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AbstractTransactionResponse {
    pub(super) result_code: ResultCode,
    pub(super) msg: Option<String>,
    pub(super) transaction_exception_code: TransactionExceptionCode,
}

impl AbstractTransactionResponse {
    /// 输入字段游标，返回完整响应前缀；失败消息使用 short 长度。
    pub(super) fn read(reader: &mut WireReader<'_>) -> io::Result<Self> {
        let result_code = ResultCode::from_code(reader.u8()?)?;
        let msg = if result_code == ResultCode::Failed {
            reader.optional16()?
        } else {
            None
        };
        let transaction_exception_code = TransactionExceptionCode::from_code(reader.u8()?)?;
        Ok(Self {
            result_code,
            msg,
            transaction_exception_code,
        })
    }

    /// 输入写入器，写出前缀；成功结果不能携带线缆无法表达的失败消息。
    pub(super) fn write(&self, writer: &mut WireWriter) -> io::Result<()> {
        if self.result_code == ResultCode::Success && self.msg.is_some() {
            return Err(crate::invalid("success response cannot carry failure text"));
        }
        writer.u8(self.result_code as u8)?;
        if self.result_code == ResultCode::Failed {
            writer.optional16(self.msg.as_deref())?;
        }
        writer.u8(self.transaction_exception_code as u8)
    }
}
