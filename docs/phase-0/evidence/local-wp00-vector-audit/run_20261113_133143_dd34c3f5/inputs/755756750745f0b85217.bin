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

use super::branch_commit_request::BranchCommitRequest;
use super::branch_commit_response::BranchCommitResponse;
use super::branch_register_request::BranchRegisterRequest;
use super::branch_register_response::BranchRegisterResponse;
use super::branch_report_request::BranchReportRequest;
use super::branch_report_response::BranchReportResponse;
use super::branch_rollback_request::BranchRollbackRequest;
use super::branch_rollback_response::BranchRollbackResponse;
use super::global_begin_request::GlobalBeginRequest;
use super::global_begin_response::GlobalBeginResponse;
use super::global_commit_request::GlobalCommitRequest;
use super::global_commit_response::GlobalCommitResponse;
use super::global_lock_query_request::GlobalLockQueryRequest;
use super::global_lock_query_response::GlobalLockQueryResponse;
use super::global_report_request::GlobalReportRequest;
use super::global_report_response::GlobalReportResponse;
use super::global_rollback_request::GlobalRollbackRequest;
use super::global_rollback_response::GlobalRollbackResponse;
use super::global_status_request::GlobalStatusRequest;
use super::global_status_response::GlobalStatusResponse;
use super::register_rm_request::RegisterRMRequest;
use super::register_rm_response::RegisterRMResponse;
use super::register_tm_request::RegisterTMRequest;
use super::register_tm_response::RegisterTMResponse;
use super::undo_log_delete_request::UndoLogDeleteRequest;
use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;
use std::io;

/// 独立验证工具的显式载荷分派，不缓存输入字节，不代表正式协议 API。
/// 对应 Java: org.apache.seata.serializer.seata.SeataSerializer 的类型分派职责。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PayloadMessage {
    GlobalBeginRequest(GlobalBeginRequest),
    GlobalBeginResponse(GlobalBeginResponse),
    BranchCommitRequest(BranchCommitRequest),
    BranchCommitResponse(BranchCommitResponse),
    BranchRollbackRequest(BranchRollbackRequest),
    BranchRollbackResponse(BranchRollbackResponse),
    GlobalCommitRequest(GlobalCommitRequest),
    GlobalCommitResponse(GlobalCommitResponse),
    GlobalRollbackRequest(GlobalRollbackRequest),
    GlobalRollbackResponse(GlobalRollbackResponse),
    BranchRegisterRequest(BranchRegisterRequest),
    BranchRegisterResponse(BranchRegisterResponse),
    BranchReportRequest(BranchReportRequest),
    BranchReportResponse(BranchReportResponse),
    GlobalStatusRequest(GlobalStatusRequest),
    GlobalStatusResponse(GlobalStatusResponse),
    GlobalReportRequest(GlobalReportRequest),
    GlobalReportResponse(GlobalReportResponse),
    GlobalLockQueryRequest(GlobalLockQueryRequest),
    GlobalLockQueryResponse(GlobalLockQueryResponse),
    RegisterTMRequest(RegisterTMRequest),
    RegisterTMResponse(RegisterTMResponse),
    RegisterRMRequest(RegisterRMRequest),
    RegisterRMResponse(RegisterRMResponse),
    UndoLogDeleteRequest(UndoLogDeleteRequest),
}

impl PayloadMessage {
    /// 输入含双字节类型前缀的完整载荷，返回对象；未知类型、截断、尾部数据均拒绝。
    pub(super) fn decode(input: &[u8]) -> io::Result<Self> {
        let mut reader = WireReader::new(input)?;
        let message = Self::read_prefix(&mut reader)?;
        reader.finish()?;
        Ok(message)
    }

    /// 输入有界嵌套游标，只读取一个普通类型对象；容器负责最后的长度检查。
    pub(super) fn read_prefix(reader: &mut WireReader<'_>) -> io::Result<Self> {
        let code = reader.u16()?;
        let message = match code {
            1 => Self::GlobalBeginRequest(GlobalBeginRequest::read(reader)?),
            2 => Self::GlobalBeginResponse(GlobalBeginResponse::read(reader)?),
            3 => Self::BranchCommitRequest(BranchCommitRequest::read(reader)?),
            4 => Self::BranchCommitResponse(BranchCommitResponse::read(reader)?),
            5 => Self::BranchRollbackRequest(BranchRollbackRequest::read(reader)?),
            6 => Self::BranchRollbackResponse(BranchRollbackResponse::read(reader)?),
            7 => Self::GlobalCommitRequest(GlobalCommitRequest::read(reader)?),
            8 => Self::GlobalCommitResponse(GlobalCommitResponse::read(reader)?),
            9 => Self::GlobalRollbackRequest(GlobalRollbackRequest::read(reader)?),
            10 => Self::GlobalRollbackResponse(GlobalRollbackResponse::read(reader)?),
            11 => Self::BranchRegisterRequest(BranchRegisterRequest::read(reader)?),
            12 => Self::BranchRegisterResponse(BranchRegisterResponse::read(reader)?),
            13 => Self::BranchReportRequest(BranchReportRequest::read(reader)?),
            14 => Self::BranchReportResponse(BranchReportResponse::read(reader)?),
            15 => Self::GlobalStatusRequest(GlobalStatusRequest::read(reader)?),
            16 => Self::GlobalStatusResponse(GlobalStatusResponse::read(reader)?),
            17 => Self::GlobalReportRequest(GlobalReportRequest::read(reader)?),
            18 => Self::GlobalReportResponse(GlobalReportResponse::read(reader)?),
            21 => Self::GlobalLockQueryRequest(GlobalLockQueryRequest::read(reader)?),
            22 => Self::GlobalLockQueryResponse(GlobalLockQueryResponse::read(reader)?),
            101 => Self::RegisterTMRequest(RegisterTMRequest::read(reader)?),
            102 => Self::RegisterTMResponse(RegisterTMResponse::read(reader)?),
            103 => Self::RegisterRMRequest(RegisterRMRequest::read(reader)?),
            104 => Self::RegisterRMResponse(RegisterRMResponse::read(reader)?),
            111 => Self::UndoLogDeleteRequest(UndoLogDeleteRequest::read(reader)?),
            _ => return Err(crate::invalid(format!("unsupported payload type: {code}"))),
        };
        Ok(message)
    }

    /// 无参数，独立根据对象字段重新编码类型与消息体；返回字节或长度/字段错误。
    pub(super) fn encode(&self) -> io::Result<Vec<u8>> {
        let mut writer = WireWriter::default();
        match self {
            Self::GlobalBeginRequest(message) => {
                writer.u16(1)?;
                message.write(&mut writer)?;
            }
            Self::GlobalBeginResponse(message) => {
                writer.u16(2)?;
                message.write(&mut writer)?;
            }
            Self::BranchCommitRequest(message) => {
                writer.u16(3)?;
                message.write(&mut writer)?;
            }
            Self::BranchCommitResponse(message) => {
                writer.u16(4)?;
                message.write(&mut writer)?;
            }
            Self::BranchRollbackRequest(message) => {
                writer.u16(5)?;
                message.write(&mut writer)?;
            }
            Self::BranchRollbackResponse(message) => {
                writer.u16(6)?;
                message.write(&mut writer)?;
            }
            Self::GlobalCommitRequest(message) => {
                writer.u16(7)?;
                message.write(&mut writer)?;
            }
            Self::GlobalCommitResponse(message) => {
                writer.u16(8)?;
                message.write(&mut writer)?;
            }
            Self::GlobalRollbackRequest(message) => {
                writer.u16(9)?;
                message.write(&mut writer)?;
            }
            Self::GlobalRollbackResponse(message) => {
                writer.u16(10)?;
                message.write(&mut writer)?;
            }
            Self::BranchRegisterRequest(message) => {
                writer.u16(11)?;
                message.write(&mut writer)?;
            }
            Self::BranchRegisterResponse(message) => {
                writer.u16(12)?;
                message.write(&mut writer)?;
            }
            Self::BranchReportRequest(message) => {
                writer.u16(13)?;
                message.write(&mut writer)?;
            }
            Self::BranchReportResponse(message) => {
                writer.u16(14)?;
                message.write(&mut writer)?;
            }
            Self::GlobalStatusRequest(message) => {
                writer.u16(15)?;
                message.write(&mut writer)?;
            }
            Self::GlobalStatusResponse(message) => {
                writer.u16(16)?;
                message.write(&mut writer)?;
            }
            Self::GlobalReportRequest(message) => {
                writer.u16(17)?;
                message.write(&mut writer)?;
            }
            Self::GlobalReportResponse(message) => {
                writer.u16(18)?;
                message.write(&mut writer)?;
            }
            Self::GlobalLockQueryRequest(message) => {
                writer.u16(21)?;
                message.write(&mut writer)?;
            }
            Self::GlobalLockQueryResponse(message) => {
                writer.u16(22)?;
                message.write(&mut writer)?;
            }
            Self::RegisterTMRequest(message) => {
                writer.u16(101)?;
                message.write(&mut writer)?;
            }
            Self::RegisterTMResponse(message) => {
                writer.u16(102)?;
                message.write(&mut writer)?;
            }
            Self::RegisterRMRequest(message) => {
                writer.u16(103)?;
                message.write(&mut writer)?;
            }
            Self::RegisterRMResponse(message) => {
                writer.u16(104)?;
                message.write(&mut writer)?;
            }
            Self::UndoLogDeleteRequest(message) => {
                writer.u16(111)?;
                message.write(&mut writer)?;
            }
        }
        Ok(writer.finish())
    }
}
